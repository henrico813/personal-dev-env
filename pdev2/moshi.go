package main

import (
	"errors"
	"os"
	"os/exec"
	"strings"
	"syscall"
)

// The phone question is sent by this same program, started again with
// --moshi-ask. Tests replace moshiProgram so they never message a phone.
var moshiProgram = os.Executable

// startMoshi sends the phone question unless the one saved in the record is
// still running, so reruns of a waiting request never send a second question.
// The returned record carries the saved PID, so later saves under the same
// lock keep it.
func startMoshi(rec record) record {
	if rec.MoshiPID != 0 && processAlive(rec.MoshiPID) {
		return rec
	}
	path, err := moshiProgram()
	if err != nil {
		return rec
	}
	cmd := exec.Command(path, "--moshi-ask", rec.ID)
	// When the agent's command ends, the tool that ran it stops every program
	// the command started (its process group). You may answer hours later, so
	// the question is started apart from that group, where the stop does not
	// reach it, and is not waited for.
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		return rec
	}
	rec.MoshiPID = cmd.Process.Pid
	// Reruns check this PID before sending a question. If it cannot be saved,
	// stop the question, or a rerun would send a second one.
	if err := saveKeepingAge(rec); err != nil {
		_ = cmd.Process.Kill()
		rec.MoshiPID = 0
	}
	return rec
}

func processAlive(pid int) bool { return syscall.Kill(pid, 0) == nil }

// Ends a phone question that can no longer decide anything, so it does not
// stay on the phone for hours. The question runs in its own session, so its
// PID is also its process group ID. moshiAsk must not call this for its own
// answer.
func stopMoshi(pid int) {
	if pid != 0 && processAlive(pid) {
		_ = syscall.Kill(-pid, syscall.SIGTERM)
	}
}

// moshiAsk runs as pde-gh-write --moshi-ask ID. It waits up to four hours for
// the phone answer and saves it only if the request is still pending; a later
// answer changes nothing. It never runs gh: the request's runner, the waiting
// pde-gh-write, or a rerun does.
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
	// Remove this program's PID whenever it ends while the request is live. The
	// system reuses PIDs, so a stale one could name an unrelated program that
	// looks like a running question, stop reruns from sending a new one, and
	// be signaled by stopMoshi.
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
