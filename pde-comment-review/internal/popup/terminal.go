package popup

import (
	"os"
	"strings"
	"syscall"
	"unicode/utf8"
	"unsafe"
)

const (
	ioctlFlush   = 0x540b // TCFLSH
	flushPending = 0      // TCIFLUSH
)

// The screen's terminal in raw mode, where each key reaches the program as
// soon as it is pressed, without echo.
type Terminal struct {
	in, out *os.File
	old     syscall.Termios
	pending []byte // keys read but not yet returned
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
	if _, err := t.out.WriteString("\x1b[?25l"); err != nil {
		t.Restore()
		return err
	}
	return nil
}

// Restores the terminal's old settings, shows the cursor and clears the
// screen.
func (t *Terminal) Restore() {
	_, _ = t.out.WriteString("\x1b[?25h\x1b[H\x1b[2J")
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
		buf := make([]byte, 64)
		n, err := syscall.Read(int(t.in.Fd()), buf)
		if err == syscall.EINTR {
			return "", nil
		}
		if err != nil {
			return "", err
		}
		if n == 0 || buf[0] == 0x1b {
			return "", nil
		}
		t.pending = buf[:n]
	}
	r, size := utf8.DecodeRune(t.pending)
	t.pending = t.pending[size:]
	return string(r), nil
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
