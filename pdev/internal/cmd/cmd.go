package cmd

import (
	"io"
	"os"

	"pdev/internal/approval"
)

const (
	exitFailed   = 1
	exitRejected = 2
)

// Main runs the mode that args picks and returns the exit code. args
// includes the program name, as os.Args does.
func Main(args []string) int {
	if len(args) == 3 {
		switch args[1] {
		// pde-gh starts these two detached, beside itself.
		case "--moshi-ask":
			return moshiAsk(args[2:])
		case "--herdr-notify":
			return notifyHerdr(args[2])
		}
	}
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

var stderr io.Writer = os.Stderr

// pde-gh runs approved requests. Tests replace runnerStart so they never
// start it.
var runnerStart = startGHRunner

func startGHRunner(rec approval.Record) approval.Record {
	path, err := approval.FindSibling("pde-gh")
	if err != nil {
		return rec
	}
	return approval.StartRunner(rec, path)
}

// Restarts a runner that was killed or lost on reboot, so a late approval
// still runs. Callers hold the request lock.
func ensureRunner(rec approval.Record) approval.Record {
	if rec.RunnerPID == 0 || !approval.ProcessAlive(rec.RunnerPID) {
		return runnerStart(rec)
	}
	return rec
}

func isExecutable(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir() && info.Mode()&0o111 != 0
}
