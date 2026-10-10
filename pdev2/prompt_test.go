package main

import (
	"bufio"
	"os"
	"strconv"
	"syscall"
	"testing"
	"time"
	"unsafe"
)

func openPTY(t *testing.T) (master, terminal *os.File) {
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

// The old bash prompt approved on one stray key. Input typed before the question
// and a lone "y" must not decide.
func TestPromptNeedsDeliberateAnswer(t *testing.T) {
	cases := []struct{ name, typedEarly, typed, want string }{
		{name: "y is not yes", typed: "y\nyes\n", want: "yes"},
		{name: "no declines", typed: "no\n", want: "no"},
		{name: "typed early is discarded", typedEarly: "yes\n", typed: "no\n", want: "no"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			// Writing to the pseudo-terminal's master side is typing into it.
			// Early input lands before the prompt starts and must be discarded.
			master, terminal := openPTY(t)
			if _, err := master.WriteString(tc.typedEarly); err != nil {
				t.Fatal(err)
			}
			result := make(chan string, 1)
			go func() { choice, _ := askYesNo(terminal); result <- choice }()

			// Type the answer only after the question is printed; earlier
			// input would be discarded too.
			waitForQuestion(t, master)
			if _, err := master.WriteString(tc.typed); err != nil {
				t.Fatal(err)
			}

			select {
			case got := <-result:
				if got != tc.want {
					t.Errorf("answer = %q, want %q", got, tc.want)
				}
			case <-time.After(2 * time.Second):
				t.Fatal("prompt did not return")
			}
		})
	}
}

// A closed terminal is not consent; any answer here would record a decision
// nobody typed.
func TestPromptGivesNoAnswerWhenClosed(t *testing.T) {
	master, terminal := openPTY(t)
	result := make(chan string, 1)
	go func() { choice, _ := askYesNo(terminal); result <- choice }()

	// Close the terminal once the prompt is waiting for input.
	waitForQuestion(t, master)
	master.Close()

	select {
	case got := <-result:
		if got != "" {
			t.Errorf("answer = %q, want none", got)
		}
	case <-time.After(2 * time.Second):
		t.Fatal("prompt did not return")
	}
}

func waitForQuestion(t *testing.T, master *os.File) {
	t.Helper()
	asked := make(chan struct{})
	go func() {
		reader := bufio.NewReader(master)
		for {
			char, err := reader.ReadByte()
			if err != nil || char == '?' {
				close(asked)
				return
			}
		}
	}()
	select {
	case <-asked:
	case <-time.After(2 * time.Second):
		t.Fatal("prompt never asked")
	}
}
