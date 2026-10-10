package main

import (
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

// Agent commands are killed after about two minutes, so the wait must end first.
var answerTimeout = 90 * time.Second

var stderr io.Writer = os.Stderr

func requestWrite(args []string) int {
	req, err := parse(args, os.Stdin)
	if err != nil {
		fmt.Fprintf(stderr, "pde-gh-write: %v\n", err)
		return exitRejected
	}
	gh := findRealGH()
	if gh == "" {
		return printError(errors.New("cannot find the real gh executable"))
	}
	state, err := loadOrCreate(req, gh)
	if err != nil {
		return printError(err)
	}
	if state == statePending {
		return waitForAnswer(req.ID, gh)
	}
	return reportOrRun(req.ID, gh)
}

// The lock stops two identical first requests from both creating a record;
// the later save could overwrite an approval or a finished run.
func loadOrCreate(req request, gh string) (string, error) {
	lockFile, err := lockRequest(req.ID)
	if err != nil {
		return "", err
	}
	defer lockFile.Close()
	rec, err := load(req.ID)
	// An expired record of any state is replaced, so after four hours the same
	// command needs a new approval and may run again.
	if err == nil && !expired(rec) {
		if rec.State == statePending {
			// startMoshi does nothing while the earlier ask is alive, so this only
			// restarts an ask that died. Otherwise a failed ask would leave a
			// pending request with no question on the phone.
			startMoshi(rec, moshiQuestion(req))
		}
		return rec.State, nil
	}
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return "", err
	}
	text, err := approvalText(gh, req)
	if err != nil {
		return "", err
	}
	rec = record{ID: req.ID, Args: req.Args, Title: req.Title, Body: req.Body,
		BodyFile: req.BodyFile, ApprovalText: text, State: statePending}
	if err := save(rec); err != nil {
		return "", err
	}
	fmt.Fprintf(stderr, "pde-gh-write: waiting for approval in the Herdr popup or on the phone, or run pde-pr-approve %s in a terminal\n", req.ID)
	// A slow Herdr must not delay the wait loop; the process may exit before this finishes.
	go notifyRequest(rec)
	startMoshi(rec, moshiQuestion(req))
	return statePending, nil
}

func waitForAnswer(id, gh string) int {
	deadline := time.Now().Add(answerTimeout)
	for time.Now().Before(deadline) {
		if rec, err := load(id); err == nil && rec.State != statePending {
			return reportOrRun(id, gh)
		}
		time.Sleep(50 * time.Millisecond)
	}
	return reportOrRun(id, gh)
}

// The lock stays held while gh runs. A running record found under the lock
// therefore means that run died, and it is never rerun.
func reportOrRun(id, gh string) int {
	lockFile, err := lockRequest(id)
	if err != nil {
		return printError(err)
	}
	defer lockFile.Close()
	rec, err := load(id)
	if err != nil {
		return printError(err)
	}
	switch rec.State {
	case stateApproved:
		return runGH(rec, gh)
	case stateRan:
		fmt.Fprintf(stderr, "pde-gh-write: already ran %s, exit %d\n", id, rec.Exit)
		return rec.Exit
	case stateDeclined:
		fmt.Fprintf(stderr, "pde-gh-write: declined %s\n", id)
		return exitDeclined
	case stateRunning:
		fmt.Fprintf(stderr, "pde-gh-write: interrupted %s\n", id)
		return exitInterrupted
	}
	fmt.Fprintf(stderr, "pde-gh-write: approval needed: run pde-pr-approve %s in a terminal, then rerun this exact command\n", id)
	return exitWaiting
}

func printError(err error) int {
	fmt.Fprintf(stderr, "pde-gh-write: %v\n", err)
	return exitFailed
}

// The first gh on PATH is the guard, a wrapper that blocks PR writes unless
// PDE_GH_WRITE=1 is set. The real gh is the next executable gh on PATH that
// is neither the guard nor this program.
func findRealGH() string {
	self, _ := os.Executable()
	self, _ = filepath.EvalSymlinks(self)
	guard, _ := exec.LookPath("gh")
	guard, _ = filepath.EvalSymlinks(guard)
	for _, dir := range strings.Split(os.Getenv("PATH"), ":") {
		if dir == "" {
			dir = "."
		}
		path, err := filepath.EvalSymlinks(filepath.Join(dir, "gh"))
		if err == nil && path != self && path != guard && isExecutable(path) {
			return path
		}
	}
	return ""
}

func isExecutable(path string) bool {
	info, err := os.Stat(path)
	return err == nil && !info.IsDir() && info.Mode()&0o111 != 0
}

func runGH(rec record, gh string) int {
	rec.State = stateRunning
	if err := save(rec); err != nil {
		return printError(err)
	}
	cmd := exec.Command(gh, rec.Args...)
	cmd.Stdout, cmd.Stderr = os.Stdout, os.Stderr
	if rec.BodyFile {
		cmd.Stdin = strings.NewReader(rec.Body)
	}
	// Lets this one approved command through the gh guard.
	cmd.Env = append(os.Environ(), "PDE_GH_WRITE=1")
	rec.Exit = 0
	if err := cmd.Run(); err != nil {
		var exitErr *exec.ExitError
		rec.Exit = 1
		if errors.As(err, &exitErr) {
			rec.Exit = exitErr.ExitCode()
		}
	}
	rec.State = stateRan
	if err := save(rec); err != nil {
		// The record stays running, so reruns report interrupted and never rerun.
		fmt.Fprintf(stderr, "pde-gh-write: gh exited %d but the result was not saved: %v\n", rec.Exit, err)
	}
	return rec.Exit
}
