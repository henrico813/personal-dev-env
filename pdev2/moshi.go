package main

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"syscall"
)

// The phone question is sent by this same program, started again with
// --moshi-ask. Tests replace moshiProgram so they never message a phone.
var moshiProgram = os.Executable

// startMoshi sends the phone question unless the one saved in the record is
// still running, so reruns of a waiting request never send a second question.
func startMoshi(rec record, question string) {
	if rec.MoshiPID != 0 && processAlive(rec.MoshiPID) {
		return
	}
	path, err := moshiProgram()
	if err != nil {
		return
	}
	cmd := exec.Command(path, "--moshi-ask", rec.ID, question)
	// When the agent's command ends, the tool that ran it stops every program
	// the command started (its process group). You may answer hours later, so
	// the question is started apart from that group, where the stop does not
	// reach it, and is not waited for.
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		return
	}
	rec.MoshiPID = cmd.Process.Pid
	// Reruns check this PID before sending a question. If it cannot be saved,
	// stop the question, or a rerun would send a second one.
	if err := saveKeepingAge(rec); err != nil {
		_ = cmd.Process.Kill()
	}
}

func processAlive(pid int) bool { return syscall.Kill(pid, 0) == nil }

// moshiAsk runs as pde-gh-write --moshi-ask ID QUESTION. It waits up to four
// hours for the phone answer and saves it only if the request is still
// pending; a later answer changes nothing. It never runs gh: the waiting
// pde-gh-write or a rerun does.
func moshiAsk(args []string) int {
	if len(args) != 2 {
		return 1
	}
	commandErr := exec.Command("moshi-hook", "ask", "--require-remote", "--source", "pde-gh-write", "--timeout", "4h", args[1]).Run()
	// moshi-hook ask exits 0 for yes and 1 for no; any other result, such as a
	// timeout or a killed helper, is no answer.
	answer := ""
	var exitErr *exec.ExitError
	if commandErr == nil {
		answer = "yes"
	} else if errors.As(commandErr, &exitErr) && exitErr.ExitCode() == 1 {
		answer = "no"
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
	// Remove this program's PID whenever it ends while the request is live. The
	// system reuses PIDs, so a stale one could name an unrelated program that
	// looks like a running question and stops reruns from sending a new one.
	owned := rec.MoshiPID == os.Getpid()
	answered := answer != "" && rec.State == statePending
	if !owned && !answered {
		return 0
	}
	rec.MoshiPID = 0
	if !answered {
		if err := saveKeepingAge(rec); err != nil {
			return 1
		}
		return 0
	}
	rec.State = stateDeclined
	if answer == "yes" {
		rec.State = stateApproved
	}
	if err := save(rec); err != nil {
		return 1
	}
	return 0
}

func moshiQuestion(req request) string {
	repo, target := req.Repo, req.Target
	if repo == "" {
		repo = "current repository"
	}
	if target == "" {
		target = "the requested PR"
	}
	question := fmt.Sprintf("Approve gh pr %s in %s (%s)", req.Operation, repo, target)
	if req.Title != "" {
		question += ", titled " + req.Title
	}
	return question + "?"
}
