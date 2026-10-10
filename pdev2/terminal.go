package main

import (
	"os"
	"os/exec"
	"strings"
	"syscall"
	"unsafe"
)

const (
	ioctlFlush   = 0x540b // TCFLSH
	flushPending = 0      // TCIFLUSH
)

func showPager(tty *os.File, text string) error {
	pager := exec.Command("less")
	pager.Stdin = strings.NewReader(text)
	pager.Stdout, pager.Stderr = tty, tty
	return pager.Run()
}

func terminalSize(file *os.File) (int, int) {
	var size struct{ rows, cols, x, y uint16 }
	_, _, _ = syscall.Syscall(syscall.SYS_IOCTL, file.Fd(), syscall.TIOCGWINSZ, uintptr(unsafe.Pointer(&size)))
	return int(size.cols), int(size.rows)
}

func setRaw(file *os.File) (*syscall.Termios, error) {
	var old syscall.Termios
	if err := ioctl(file, syscall.TCGETS, &old); err != nil {
		return nil, err
	}
	raw := old
	raw.Lflag &^= syscall.ICANON | syscall.ECHO
	raw.Iflag &^= syscall.ISTRIP | syscall.INLCR | syscall.IXON
	raw.Oflag &^= syscall.OPOST
	if err := ioctl(file, syscall.TCSETS, &raw); err != nil {
		return nil, err
	}
	return &old, nil
}

func restore(file *os.File, old *syscall.Termios) { _ = ioctl(file, syscall.TCSETS, old) }

func ioctl(file *os.File, request uintptr, termios *syscall.Termios) error {
	_, _, errno := syscall.Syscall(syscall.SYS_IOCTL, file.Fd(), request, uintptr(unsafe.Pointer(termios)))
	if errno != 0 {
		return errno
	}
	return nil
}

// flushInput discards input typed but not yet read.
func flushInput(file *os.File) error {
	_, _, errno := syscall.Syscall(syscall.SYS_IOCTL, file.Fd(), ioctlFlush, uintptr(flushPending))
	if errno != 0 {
		return errno
	}
	return nil
}
