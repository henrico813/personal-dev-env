package main

import (
	"io"
	"os"
	"os/exec"
	"syscall"
	"time"
)

// The runner is a detached copy of this binary, run as --pde-runner ID and
// started with each request. It outlives the agent's 90-second wait, runs gh
// once the request is approved, and saves the output for the agent's rerun.
// Tests replace runnerStart so they never start the test binary.
var runnerStart = startDetachedRunner

// The runner is this binary. A test that runs startDetachedRunner replaces it.
var runnerProgram = os.Executable

// Once a second is prompt enough for a person and cheap over a four-hour wait.
var runnerPoll = time.Second

// Callers hold the request lock, because the runner PID is saved into rec.
// The returned record carries the saved PID, so later saves under the same
// lock keep it.
func startDetachedRunner(rec record) record {
	path, err := runnerProgram()
	if err != nil {
		return rec
	}
	cmd := exec.Command(path, "--pde-runner", rec.ID)
	// The agent's command group is killed when the foreground wait ends. The
	// runner is not waited for, because it must outlive this command.
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		return rec
	}
	rec.RunnerPID = cmd.Process.Pid
	// Like the phone ask's PID, this is not a change of the request, so it
	// must not restart expiry.
	if err := saveKeepingAge(rec); err != nil {
		_ = cmd.Process.Kill()
		rec.RunnerPID = 0
	}
	return rec
}

// Restarts a runner that was killed or lost on reboot, so a late approval
// still runs. Callers hold the request lock.
func ensureRunner(rec record) record {
	if rec.RunnerPID == 0 || !processAlive(rec.RunnerPID) {
		return runnerStart(rec)
	}
	return rec
}

// Waits for an answer, then goes through the same locked path as the
// foreground writer, so gh runs at most once whichever process gets there first.
func runnerMain(id string) int {
	for {
		rec, err := load(id)
		if err != nil {
			return 1
		}
		if expired(rec) {
			clearRecordMarker(rec)
			stopMoshi(rec.MoshiPID)
			return 0
		}
		if rec.State != statePending {
			code := reportOrRunOutput(id, rec.GH, io.Discard)
			clearRecordMarker(rec)
			stopMoshi(rec.MoshiPID)
			return code
		}
		time.Sleep(runnerPoll)
	}
}
