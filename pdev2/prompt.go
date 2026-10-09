package main

import (
	"bufio"
	"fmt"
	"io"
	"os"
	"strings"
	"syscall"
)

const (
	ioctlFlush   = 0x540b // TCFLSH
	flushPending = 0      // TCIFLUSH
)

// Input typed before the question is discarded. EOF or a closed terminal is
// not an answer.
func askYesNo(tty *os.File) (string, error) {
	if _, _, errno := syscall.Syscall(syscall.SYS_IOCTL, tty.Fd(), ioctlFlush, flushPending); errno != 0 {
		return "", errno
	}
	fmt.Fprint(tty, "Approve this pull request? Type yes or no, then Enter: ")
	scanner := bufio.NewScanner(tty)
	for scanner.Scan() {
		choice := strings.ToLower(strings.TrimSpace(scanner.Text()))
		if choice == "yes" || choice == "no" {
			return choice, nil
		}
		fmt.Fprint(tty, "Please type yes or no, then Enter: ")
	}
	if err := scanner.Err(); err != nil {
		return "", err
	}
	return "", io.EOF
}
