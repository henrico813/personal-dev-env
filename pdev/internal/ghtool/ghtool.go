// Package ghtool is pde-gh, which refuses pull request writes as gh and asks
// for approval as pde-gh-write; read Main, guardGH and requestWrite first.
//
// It imports no pdev package but approval, and starts pde-pr-approve beside
// itself for Herdr and the phone.
package ghtool

import (
	"fmt"
	"io"
	"os"
	"path/filepath"

	"pdev/internal/approval"
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
// the program name, as os.Args does, because gh and pde-gh-guard are picked
// by name.
func Main(args []string) int {
	switch filepath.Base(args[0]) {
	case "gh", "pde-gh-guard":
		return guardGH(args[1:])
	}
	// Hidden mode for the detached runner that runnerStart starts.
	if len(args) > 1 && args[1] == approval.RunnerFlag {
		if len(args) != 3 {
			return exitFailed
		}
		return runnerMain(args[2])
	}
	return requestWrite(args[1:])
}

var stderr io.Writer = os.Stderr

func printError(err error) int {
	fmt.Fprintf(stderr, "pde-gh-write: %v\n", err)
	return exitFailed
}

// pde-pr-approve sends the phone question and talks to Herdr. Tests replace
// this so they never start it.
var approveProgram = func() (string, error) { return approval.FindSibling("pde-pr-approve") }

// Starts pde-pr-approve --moshi-ask ID unless the saved phone question still
// runs, so reruns never send a second one. Callers hold the request lock, and
// the returned record carries the saved PID for later saves under it.
func startMoshi(rec approval.Record) approval.Record {
	if rec.MoshiPID != 0 && approval.ProcessAlive(rec.MoshiPID) {
		return rec
	}
	path, err := approveProgram()
	if err != nil {
		return rec
	}
	pid, err := approval.StartDetached(path, "--moshi-ask", rec.ID)
	if err != nil {
		return rec
	}
	rec.MoshiPID = pid
	// The new question replaces a failed one, so its reason no longer applies.
	rec.MoshiError = ""
	// Reruns check this PID before sending a question. If it cannot be saved,
	// stop the question, or a rerun would send a second one.
	if err := approval.SaveKeepingAge(rec); err != nil {
		approval.StopGroup(pid)
		rec.MoshiPID = 0
	}
	return rec
}

// Starts pde-pr-approve --herdr-notify ID detached, so a slow Herdr cannot
// delay the wait for an answer.
func startHerdrNotify(id string) {
	if path, err := approveProgram(); err == nil {
		_, _ = approval.StartDetached(path, "--herdr-notify", id)
	}
}
