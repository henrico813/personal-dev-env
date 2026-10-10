package main

import (
	"errors"
	"os"
	"os/exec"
	"strings"
	"syscall"
)

// The phone ask is this binary re-run with --moshi-ask. Tests replace it.
var moshiProgram = os.Executable

// The returned record carries the saved PID, so later saves under the same lock keep it.
func startMoshi(rec record) record {
	if rec.MoshiPID != 0 && processAlive(rec.MoshiPID) {
		return rec
	}
	path, err := moshiProgram()
	if err != nil {
		return rec
	}
	cmd := exec.Command(path, "--moshi-ask", rec.ID)
	// The agent's tool runner kills the command's whole process group when the
	// command ends, so the ask runs in its own session. It is not waited for,
	// because it must outlive this command.
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		return rec
	}
	rec.MoshiPID = cmd.Process.Pid
	// The PID prevents retries from starting a second live phone question.
	if err := saveKeepingAge(rec); err != nil {
		_ = cmd.Process.Kill()
		rec.MoshiPID = 0
	}
	return rec
}

func processAlive(pid int) bool { return syscall.Kill(pid, 0) == nil }

// Ends a phone question that can no longer decide anything, so it does not
// stay on the phone for hours. The ask runs in its own session, so its PID is
// also its process group ID. moshiAsk must not call this for its own answer.
func stopMoshi(pid int) {
	if pid != 0 && processAlive(pid) {
		_ = syscall.Kill(-pid, syscall.SIGTERM)
	}
}

func moshiAsk(args []string) int {
	if len(args) != 1 {
		return 1
	}
	answer := choiceNone
	// The question comes from the saved record, so it shows what the popup shows.
	if rec, err := load(args[0]); err == nil && rec.State == statePending {
		commandErr := exec.Command("moshi-hook", "ask", "--require-remote", "--source", "pde-gh-write", "--timeout", "4h", moshiQuestion(rec)).Run()
		// moshi-hook ask exits 0 for yes and 1 for no; any other result, such as a
		// timeout or a killed helper, is no answer.
		var exitErr *exec.ExitError
		if commandErr == nil {
			answer = choiceYes
		} else if errors.As(commandErr, &exitErr) && exitErr.ExitCode() == 1 {
			answer = choiceNo
		}
	}
	lock, err := lockRequest(args[0])
	if err != nil {
		return 1
	}
	defer lock.Close()
	rec, err := load(args[0])
	if err != nil || expired(rec) {
		return 0
	}
	if answer != choiceNone && rec.State == statePending {
		if _, err := recordAnswer(rec, answer); err != nil {
			return 1
		}
		return 0
	}
	// Every exit while the request is live clears this process's PID: a reused
	// PID would look alive and stop reruns from restarting the ask, and
	// stopMoshi would signal it.
	if rec.MoshiPID != os.Getpid() {
		return 0
	}
	rec.MoshiPID = 0
	if err := saveKeepingAge(rec); err != nil {
		return 1
	}
	return 0
}

// Uses the popup's context lines, so the phone and the popup name a request the same way.
func moshiQuestion(rec record) string {
	lines := requestContext(rec)
	return "Approve " + lines[0] + "?\n" + strings.Join(lines[1:], "\n")
}
