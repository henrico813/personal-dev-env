package approval

import (
	"os"
	"slices"
	"testing"
)

// A request saved without a runner has PID 0, which ProcessAlive calls alive,
// so it must still get one or a late approval never runs. A live runner is
// kept, so a rerun starts no second runner.
func TestEnsureRunnerRestartsOnlyAbsentRunners(t *testing.T) {
	var started []int
	start := func(rec Record) Record { started = append(started, rec.RunnerPID); return rec }

	EnsureRunner(Record{ID: "none", RunnerPID: 0}, start)
	EnsureRunner(Record{ID: "live", RunnerPID: os.Getpid()}, start)

	if !slices.Equal(started, []int{0}) {
		t.Errorf("runners started for PIDs %v, want only for PID 0", started)
	}
}
