package ghtool

import (
	"io"
	"os"
	"time"

	"pdev/internal/approval"
)

// The runner is a detached copy of this binary, started with each request as
// --pde-runner ID, that runs gh once approved and saves the output for the
// agent's rerun. Tests replace runnerStart so they never start the test binary.
var runnerStart = startDetachedRunner

// The runner is this binary. A test that runs startDetachedRunner replaces it.
var runnerProgram = os.Executable

// Once a second is prompt enough for a person and cheap over a four-hour wait.
var runnerPoll = time.Second

// Callers hold the request lock, because the runner PID is saved into rec.
func startDetachedRunner(rec approval.Record) approval.Record {
	path, err := runnerProgram()
	if err != nil {
		return rec
	}
	return approval.StartRunner(rec, path)
}

// Waits for an answer, then goes through the same locked path as the
// foreground writer, so gh runs at most once whichever process gets there first.
func runnerMain(id string) int {
	for {
		rec, err := approval.Load(id)
		if err != nil {
			return 1
		}
		if approval.Expired(rec) {
			approval.StopGroup(rec.MoshiPID)
			if rec.PaneID != "" {
				startHerdrNotify(rec.ID)
			}
			return 0
		}
		if rec.State != approval.StatePending {
			return reportOrRunOutput(id, rec.GH, io.Discard)
		}
		time.Sleep(runnerPoll)
	}
}
