package app

import (
	"io"
	"os"
	"os/exec"
	"os/signal"
	"strconv"
	"strings"
	"syscall"
)

// Clears the screen, so a loading line replaces fzf's list.
const clearScreen = "\x1b[H\x1b[2J"

// Runs work with a new temporary folder and removes the folder afterwards.
// A hang-up or TERM runs stop, when given, and removes the folder too, then
// exits, so a dropped SSH connection leaves nothing behind.
func (a *prsApp) runInTempDir(
	stop func(),
	work func(dir string) int,
) int {
	dir, err := os.MkdirTemp(
		"",
		"prs-",
	)
	if err != nil {
		a.printError("prs: " + err.Error())
		return 1
	}
	signals := make(
		chan os.Signal,
		1,
	)
	signal.Notify(
		signals,
		syscall.SIGHUP,
		syscall.SIGTERM,
	)
	defer signal.Stop(signals)
	done := make(chan struct{})
	ended := make(chan struct{})
	go func() {
		defer close(ended)
		var caught os.Signal
		select {
		case <-done:
			return
		case caught = <-signals:
		}
		// Further signals are ignored, so a second one cannot stop prs
		// halfway through removing the folder.
		signal.Ignore(
			syscall.SIGHUP,
			syscall.SIGTERM,
		)
		if stop != nil {
			stop()
		}
		_ = os.RemoveAll(dir)
		// Like a shell, exit with 128 plus the signal's number.
		code := 1
		if number, ok := caught.(syscall.Signal); ok {
			code = 128 + int(number)
		}
		os.Exit(code)
	}()
	defer func() {
		close(done)
		<-ended
	}()
	defer func() { _ = os.RemoveAll(dir) }()
	return work(dir)
}

// Returns the terminal's columns, read from /dev/tty so it works when
// stdout is a pipe, or 80 when there is no terminal.
func measureScreenWidth() int {
	tty, err := os.Open("/dev/tty")
	if err != nil {
		return 80
	}
	defer func() { _ = tty.Close() }()
	size, _ := runStty(
		tty,
		"size",
	)
	fields := strings.Fields(size)
	if len(fields) != 2 {
		return 80
	}
	width, err := strconv.Atoi(fields[1])
	if err != nil || width == 0 {
		return 80
	}
	return width
}

// Reads one key from in, without waiting for Enter when in is a terminal.
func readOneKey(in io.Reader) {
	if tty, ok := in.(*os.File); ok {
		if saved, err := runStty(
			tty,
			"-g",
		); err == nil {
			_, _ = runStty(
				tty,
				"-icanon",
				"-echo",
				"min",
				"1",
			)
			defer func() {
				_, _ = runStty(
					tty,
					strings.TrimSpace(saved),
				)
			}()
		}
	}
	// An arrow key sends several bytes at once, and one read takes them
	// all, so none are left for fzf.
	_, _ = in.Read(make(
		[]byte,
		16,
	))
}

// Runs stty on tty and returns what it printed.
func runStty(
	tty *os.File,
	args ...string,
) (
	string,
	error,
) {
	cmd := exec.Command(
		"stty",
		args...,
	)
	cmd.Stdin = tty
	out, err := cmd.Output()
	return string(out), err
}

// Prints the message and waits for a key, so the message stays on screen.
func (a *prsApp) waitForKey(message string) {
	a.printText(message + " Press a key to go back. ")
	readOneKey(a.stdin)
	a.printLine("")
}
