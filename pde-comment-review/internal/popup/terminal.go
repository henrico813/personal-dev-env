package popup

import (
	"os"
	"strconv"
	"strings"
	"syscall"
	"unicode/utf8"
	"unsafe"
)

const (
	ioctlFlush   = 0x540b // TCFLSH
	flushPending = 0      // TCIFLUSH
)

// Keys that ReadKey returns for the up and down arrows and for a wheel turn.
// Notes and searches skip them, since they start with Esc.
const (
	UpKey   = "\x1b[A"
	DownKey = "\x1b[B"
)

// Turn mouse reports on and off. Mode 1000 sends taps and wheel turns, and
// mode 1006 writes each one as text such as "\x1b[<65;20;5M".
const (
	mouseOn  = "\x1b[?1000h\x1b[?1006h"
	mouseOff = "\x1b[?1006l\x1b[?1000l"
)

// Last letters of the up and down arrow keys, as in "\x1b[A".
var arrowKeys = map[byte]string{
	'A': UpKey,
	'B': DownKey,
}

// Mouse button numbers of a wheel turn. A phone app sends a swipe as these.
var wheelKeys = map[int]string{
	64: UpKey,
	65: DownKey,
}

// The screen's terminal in raw mode, where each key reaches the program as
// soon as it is pressed, without echo.
type Terminal struct {
	in, out *os.File
	old     syscall.Termios
	pending []byte // bytes read but not yet returned as keys
}

// Switches the terminal to raw mode, hides the cursor, and drops keys typed
// so far, so a key meant for another program never answers an item. Later
// failures restore the old settings.
func Start(in, out *os.File) (*Terminal, error) {
	t := &Terminal{in: in, out: out}
	if err := ioctl(in, syscall.TCGETS, &t.old); err != nil {
		return nil, err
	}
	if err := t.Resume(); err != nil {
		return nil, err
	}
	return t, nil
}

// Switches back to raw mode after another program, such as nvim, used the
// terminal, and drops the keys typed into it.
func (t *Terminal) Resume() error {
	raw := t.old
	raw.Lflag &^= syscall.ICANON | syscall.ECHO | syscall.ISIG
	raw.Iflag &^= syscall.ISTRIP | syscall.INLCR | syscall.ICRNL | syscall.IXON
	raw.Oflag &^= syscall.OPOST
	// Reads return after 0.1 s with no key, so resizes redraw.
	raw.Cc[syscall.VMIN], raw.Cc[syscall.VTIME] = 0, 1
	if err := ioctl(t.in, syscall.TCSETS, &raw); err != nil {
		return err
	}
	t.pending = nil
	_, _, errno := syscall.Syscall(syscall.SYS_IOCTL,
		t.in.Fd(), ioctlFlush, flushPending)
	if errno != 0 {
		t.Restore()
		return errno
	}
	if _, err := t.out.WriteString("\x1b[?25l" + mouseOn); err != nil {
		t.Restore()
		return err
	}
	return nil
}

// Restores the terminal's old settings, shows the cursor and clears the
// screen.
func (t *Terminal) Restore() {
	_, _ = t.out.WriteString(mouseOff + "\x1b[?25h\x1b[H\x1b[2J")
	_ = ioctl(t.in, syscall.TCSETS, &t.old)
}

// Returns the terminal's columns and rows.
func (t *Terminal) Size() (int, int) { return FindSize(t.out) }

// Returns the columns and rows of the terminal out draws on.
func FindSize(out *os.File) (cols, rows int) {
	var size struct{ rows, cols, x, y uint16 }
	_, _, _ = syscall.Syscall(syscall.SYS_IOCTL,
		out.Fd(), syscall.TIOCGWINSZ, uintptr(unsafe.Pointer(&size)))
	return int(size.cols), int(size.rows)
}

func (t *Terminal) Draw(lines []string) error {
	_, err := t.out.WriteString("\x1b[H\x1b[2J" + strings.Join(lines, "\r\n"))
	return err
}

// Returns the next key, or "" when none came within a tenth of a second.
// Input starting with Esc, such as an arrow key, is dropped so its bytes
// don't act as other keys.
func (t *Terminal) ReadKey() (string, error) {
	if len(t.pending) == 0 {
		if err := t.readMore(); err != nil || len(t.pending) == 0 {
			return "", err
		}
	}
	if t.pending[0] == 0x1b {
		return t.readEscapeKey()
	}
	r, size := utf8.DecodeRune(t.pending)
	t.pending = t.pending[size:]
	return string(r), nil
}

// Reads up to 64 more bytes of input and keeps them for ReadKey. It waits at
// most a tenth of a second and keeps nothing when no byte comes.
func (t *Terminal) readMore() error {
	buf := make([]byte, 64)
	n, err := syscall.Read(int(t.in.Fd()), buf)
	if err == syscall.EINTR {
		return nil
	}
	if err != nil {
		return err
	}
	t.pending = append(t.pending, buf[:n]...)
	return nil
}

// Returns UpKey or DownKey for an arrow key or a wheel turn, and "" for other
// input that starts with Esc, such as a tap. It reads again when a read ends
// partway through that input, so none of its bytes act as keys.
func (t *Terminal) readEscapeKey() (string, error) {
	for {
		key, size, whole := parseEscapeSequence(t.pending)
		if whole {
			t.pending = t.pending[size:]
			return key, nil
		}
		before := len(t.pending)
		if err := t.readMore(); err != nil {
			return "", err
		}
		// Nothing came for a tenth of a second, so this was a lone Esc or a cut
		// off sequence. Either way it is dropped.
		if len(t.pending) == before {
			t.pending = nil
			return "", nil
		}
	}
}

// A control sequence is Esc [, then middle bytes such as "<65;20;5", then
// one final byte such as A or M. These byte ranges come from ECMA-48.
const (
	firstMiddleByte = 0x20
	lastMiddleByte  = 0x3f
	firstFinalByte  = 0x40
	lastFinalByte   = 0x7e
)

// Returns the key for the Esc sequence that starts input, and its length in
// bytes. Returns false as whole when input ends before the sequence does.
func parseEscapeSequence(input []byte) (key string, size int, whole bool) {
	if len(input) < 2 {
		return "", 0, false
	}
	switch input[1] {
	case '[':
		return parseControlSequence(input)
	case 'O': // arrows in some terminals
		if len(input) < 3 {
			return "", 0, false
		}
		return arrowKeys[input[2]], 3, true
	}
	// Alt with a key, such as Esc q, is dropped with everything after it.
	return "", len(input), true
}

// Returns the key for the Esc [ sequence that starts input, and its length in
// bytes.
func parseControlSequence(input []byte) (key string, size int, whole bool) {
	end := 2
	for end < len(input) &&
		input[end] >= firstMiddleByte &&
		input[end] <= lastMiddleByte {
		end++
	}
	if end == len(input) {
		return "", 0, false
	}
	final, middle, size := input[end], string(input[2:end]), end+1
	switch {
	case final < firstFinalByte || final > lastFinalByte:
		// Not a control sequence after all, so drop everything read.
		return "", len(input), true
	case final == 'M' && middle == "":
		return parseOldMouseReport(input)
	case final == 'M' && strings.HasPrefix(middle, "<"):
		return findWheelKey(middle), size, true
	case middle == "":
		return arrowKeys[final], size, true
	}
	return "", size, true
}

// Returns the key for a mouse report in the old form, Esc [ M and three raw
// bytes. Each raw byte is a number plus 32, so column 65 arrives as a.
func parseOldMouseReport(input []byte) (key string, size int, whole bool) {
	if len(input) < 6 {
		return "", 0, false
	}
	return wheelKeys[int(input[3])-32], 6, true
}

// Returns UpKey or DownKey when a report such as "<65;20;5" is a wheel
// turn, and "" for a tap.
func findWheelKey(report string) string {
	button, _, _ := strings.Cut(strings.TrimPrefix(report, "<"), ";")
	number, _ := strconv.Atoi(button)
	return wheelKeys[number]
}

func ioctl(file *os.File, request uintptr, termios *syscall.Termios) error {
	_, _, errno := syscall.Syscall(syscall.SYS_IOCTL,
		file.Fd(), request, uintptr(unsafe.Pointer(termios)))
	if errno != 0 {
		return errno
	}
	return nil
}

// Asks lines for the rows that fit the terminal's size, draws them, and waits
// for a key, drawing again whenever the terminal is resized.
func (t *Terminal) DrawAndReadKey(lines func(cols, rows int) []string) (string, error) {
	drawn := false
	var drawnCols, drawnRows int
	for {
		cols, rows := t.Size()
		if !drawn || cols != drawnCols || rows != drawnRows {
			if err := t.Draw(lines(cols, rows)); err != nil {
				return "", err
			}
			drawn = true
			drawnCols, drawnRows = cols, rows
		}
		key, err := t.ReadKey()
		if err != nil || key != "" {
			return key, err
		}
	}
}
