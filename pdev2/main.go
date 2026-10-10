// Package main handles PR writes that run only after a human approves them in
// a terminal, a Herdr popup, or on the phone through Moshi.
//
// An agent runs pde-gh-write with gh pr arguments. It stores a pending record
// and waits up to 90 seconds while a human approves with pde-pr-approve ID.
// Rerunning the same command reports the outcome instead of running gh again.
package main

import (
	"os"
	"path/filepath"
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

func main() {
	// Hidden mode for the background phone ask that startMoshi starts.
	if len(os.Args) > 1 && os.Args[1] == "--moshi-ask" {
		os.Exit(moshiAsk(os.Args[2:]))
	}
	if filepath.Base(os.Args[0]) == "pde-pr-approve" {
		// Herdr passes pane values as environment variables, including the request ID.
		if len(os.Args) == 2 && os.Args[1] == "--herdr-popup" {
			os.Exit(approvePopup(os.Getenv("PDE_REQUEST_ID")))
		}
		os.Exit(approve(os.Args[1:]))
	}
	os.Exit(requestWrite(os.Args[1:]))
}
