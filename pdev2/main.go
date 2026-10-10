// Package main implements pde-gh-write and pde-pr-approve: PR writes that
// run only after a human types yes in a terminal.
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
	if filepath.Base(os.Args[0]) == "pde-pr-approve" {
		os.Exit(approve(os.Args[1:]))
	}
	os.Exit(requestWrite(os.Args[1:]))
}
