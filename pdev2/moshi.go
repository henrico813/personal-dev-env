package main

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"syscall"
)

// The phone ask is this binary re-run with --moshi-ask. Tests replace it.
var moshiProgram = os.Executable

func startMoshi(rec record, question string) {
	if rec.MoshiPID != 0 && processAlive(rec.MoshiPID) {
		return
	}
	path, err := moshiProgram()
	if err != nil {
		return
	}
	cmd := exec.Command(path, "--moshi-ask", rec.ID, question)
	// The agent's tool runner kills the command's whole process group when the
	// command ends, so the ask runs in its own session. It is not waited for,
	// because it must outlive this command.
	cmd.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := cmd.Start(); err != nil {
		return
	}
	rec.MoshiPID = cmd.Process.Pid
	// The PID prevents retries from starting a second live phone question.
	if err := saveKeepingAge(rec); err != nil {
		_ = cmd.Process.Kill()
	}
}

func processAlive(pid int) bool { return syscall.Kill(pid, 0) == nil }

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
	// Every exit while the request is live clears this process's PID: a reused
	// PID would look alive and stop reruns from restarting the ask.
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
