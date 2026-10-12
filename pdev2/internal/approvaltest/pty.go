package approvaltest

import (
	"bytes"
	"os"
	"strconv"
	"syscall"
	"testing"
	"time"
	"unsafe"
)

// OpenPTY opens a pseudo-terminal. The code under test reads keys from and
// draws on terminal; the test types into and reads the screen from master.
func OpenPTY(t *testing.T) (master, terminal *os.File) {
	t.Helper()
	master, err := os.OpenFile("/dev/ptmx", os.O_RDWR, 0)
	if err != nil {
		t.Fatal(err)
	}
	var number uint32
	var unlock int32
	if _, _, errno := syscall.Syscall(syscall.SYS_IOCTL, master.Fd(), syscall.TIOCGPTN, uintptr(unsafe.Pointer(&number))); errno != 0 {
		t.Fatal(errno)
	}
	if _, _, errno := syscall.Syscall(syscall.SYS_IOCTL, master.Fd(), syscall.TIOCSPTLCK, uintptr(unsafe.Pointer(&unlock))); errno != 0 {
		t.Fatal(errno)
	}
	terminal, err = os.OpenFile("/dev/pts/"+strconv.Itoa(int(number)), os.O_RDWR, 0)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { master.Close(); terminal.Close() })
	return master, terminal
}

// Fails if the popup answers before want appears on screen.
func ShownBeforeAnswer[T any](t *testing.T, master *os.File, want string, answered chan T) {
	t.Helper()
	shown := make(chan bool, 1)
	go func() { shown <- readsUntil(master, want) }()
	select {
	case got := <-answered:
		t.Fatalf("answered %v before showing %q", got, want)
	case ok := <-shown:
		if !ok {
			t.Fatal("popup output ended")
		}
	case <-time.After(5 * time.Second):
		t.Fatalf("%q was never shown", want)
	}
}

func WriteTerminal(t *testing.T, master *os.File, text string) {
	t.Helper()
	if _, err := master.WriteString(text); err != nil {
		t.Fatal(err)
	}
}

// ReadUntil without t, for use from a goroutine.
func readsUntil(master *os.File, want string) bool {
	buf := make([]byte, 1)
	var got bytes.Buffer
	for !bytes.Contains(got.Bytes(), []byte(want)) {
		if _, err := master.Read(buf); err != nil {
			return false
		}
		got.Write(buf)
	}
	return true
}

func SetPopupSize(t *testing.T, terminal *os.File, cols, rows uint16) {
	t.Helper()
	size := struct{ rows, cols, x, y uint16 }{rows: rows, cols: cols}
	if _, _, errno := syscall.Syscall(syscall.SYS_IOCTL, terminal.Fd(), syscall.TIOCSWINSZ, uintptr(unsafe.Pointer(&size))); errno != 0 {
		t.Fatal(errno)
	}
}

func PopupTerminal(t *testing.T, terminal *os.File) *os.File {
	t.Helper()
	fd, err := syscall.Dup(int(terminal.Fd()))
	if err != nil {
		t.Fatal(err)
	}
	popup := os.NewFile(uintptr(fd), "popup-terminal")
	// The popup gets its own copy because each *os.File closes its descriptor;
	// sharing one number could later close a reused one, such as a request lock.
	t.Cleanup(func() { _ = popup.Close() })
	return popup
}

// ReadUntil reads the popup's screen until want appears, failing after 2
// seconds. Tests wait for the drawn screen before typing, because the popup
// throws away anything typed before it draws.
func ReadUntil(t *testing.T, master *os.File, want string) {
	t.Helper()
	found := make(chan error, 1)
	go func() {
		buf := make([]byte, 1)
		var got bytes.Buffer
		for !bytes.Contains(got.Bytes(), []byte(want)) {
			if _, err := master.Read(buf); err != nil {
				found <- err
				return
			}
			got.Write(buf)
		}
		found <- nil
	}()
	select {
	case err := <-found:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(2 * time.Second):
		t.Fatalf("terminal never showed %q", want)
	}
}

// Receive fails the test if nothing arrives within 2 seconds.
func Receive[T any](t *testing.T, values <-chan T) T {
	t.Helper()
	select {
	case value := <-values:
		return value
	case <-time.After(2 * time.Second):
		t.Fatal("timed out after 2s")
	}
	var zero T
	return zero
}
