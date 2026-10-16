package popup

import (
	"os"
	"strings"
	"syscall"
	"unicode/utf8"
	"unsafe"
)

// The screen's terminal in raw mode, where each key reaches the program as
// soon as it is pressed, without echo.
type Terminal struct {
	in, out *os.File
	old     syscall.Termios
	pending []byte // keys read but not yet returned
}

// Switches the terminal to raw mode and hides the cursor. If hiding the
// cursor fails, the old settings are restored first.
func Start(in, out *os.File) (*Terminal, error) {
	t := &Terminal{in: in, out: out}
	if err := ioctl(in, syscall.TCGETS, &t.old); err != nil {
		return nil, err
	}
	raw := t.old
	raw.Lflag &^= syscall.ICANON | syscall.ECHO | syscall.ISIG
	raw.Iflag &^= syscall.ISTRIP | syscall.INLCR | syscall.ICRNL | syscall.IXON
	raw.Oflag &^= syscall.OPOST
	// Reads return after 0.1 s with no key, so resizes redraw.
	raw.Cc[syscall.VMIN], raw.Cc[syscall.VTIME] = 0, 1
	if err := ioctl(in, syscall.TCSETS, &raw); err != nil {
		return nil, err
	}
	if _, err := out.WriteString("\x1b[?25l"); err != nil {
		t.Restore()
		return nil, err
	}
	return t, nil
}

// Restores the terminal's old settings, shows the cursor and clears the
// screen.
func (t *Terminal) Restore() {
	_, _ = t.out.WriteString("\x1b[?25h\x1b[H\x1b[2J")
	_ = ioctl(t.in, syscall.TCSETS, &t.old)
}

// Returns the terminal's columns and rows.
func (t *Terminal) Size() (int, int) {
	var size struct{ rows, cols, x, y uint16 }
	_, _, _ = syscall.Syscall(syscall.SYS_IOCTL,
		t.out.Fd(), syscall.TIOCGWINSZ, uintptr(unsafe.Pointer(&size)))
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
