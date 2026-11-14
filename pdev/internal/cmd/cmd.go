package cmd

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"syscall"
)

// gh's own exit codes pass through and can also be 1, 2, or 4. Only this
// program's stderr lines start with "pde-gh-write:", which tells the two apart.
const (
	exitFailed      = 1
	exitRejected    = 2
	exitWaiting     = 3
	exitDeclined    = 4
	exitInterrupted = 6
)

// Main runs the mode that args picks and returns the exit code. args includes
// the program name, as os.Args does, because pde-pr-approve is picked by name.
func Main(args []string) int {
	// Hidden mode for the detached runner that runnerStart starts.
	if len(args) > 1 && args[1] == "--pde-runner" {
		if len(args) != 3 {
			return 1
		}
		return runnerMain(args[2])
	}
	// pde-gh-write starts itself this way to send the phone question; see
	// startMoshi.
	if len(args) > 1 && args[1] == "--moshi-ask" {
		return moshiAsk(args[2:])
	}
	if filepath.Base(args[0]) == "pde-pr-approve" {
		// Herdr runs this in the popup and passes the request ID in
		// PDE_REQUEST_ID.
		if len(args) == 2 && args[1] == "--herdr-popup" {
			return approvePopup(os.Getenv("PDE_REQUEST_ID"))
		}
		if len(args) == 2 && args[1] == "--herdr-inbox" {
			return openInbox()
		}
		return approve(args[1:])
	}
	return requestWrite(args[1:])
}

var stderr io.Writer = os.Stderr

func printError(err error) int {
	fmt.Fprintf(stderr, "pde-gh-write: %v\n", err)
	return exitFailed
}

func processAlive(pid int) bool { return syscall.Kill(pid, 0) == nil }
