package main

import (
	"bytes"
	"context"
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
	req = scopeRequest(req)
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
// the later save could overwrite an approval or a finished run. A new request
// also starts the Herdr notification, the phone ask, and the runner; a pending
// one restarts an ask or runner that died.
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
			rec = startMoshi(rec)
			rec = ensureRunner(rec)
		}
		return rec.State, nil
	}
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return "", err
	}
	if req.Repo == "" {
		// Looked up only after the ID is made, so a failed lookup on a rerun
		// still finds the same request.
		req.Repo = ghLookup(gh, req.Directory, "repo", "view", "--json", "nameWithOwner", "--jq", ".nameWithOwner")
	}
	text, fetchedTitle, err := approvalText(gh, req)
	if err != nil {
		return "", err
	}
	rec = record{ID: req.ID, Args: req.Args, Title: req.Title, Body: req.Body,
		BodyFile: req.BodyFile, ApprovalText: text, State: statePending,
		Created: time.Now(), Directory: req.Directory, Repo: req.Repo, GH: gh,
		PRTitle: currentTitle(gh, req, fetchedTitle), SessionID: os.Getenv("OPENCODE_SESSION_ID")}
	// Filled in before the first save, because a later save outside this lock
	// could overwrite an answer.
	rec.PaneID, rec.Workspace, rec.SessionTitle = agentContext(rec.SessionID)
	if err := save(rec); err != nil {
		return "", err
	}
	fmt.Fprintf(stderr, "pde-gh-write: waiting for approval in the Herdr popup or on the phone, or run pde-pr-approve %s in a terminal; it runs once approved, so rerun this exact command to read the result\n", req.ID)
	// A slow Herdr must not delay the wait loop; the process may exit before this finishes.
	go notifyRequest(rec)
	// runnerStart saves rec, so it needs the ask PID that startMoshi saved.
	rec = startMoshi(rec)
	runnerStart(rec)
	return statePending, nil
}

// The same command in another directory is a different request, so an
// approval never carries over between checkouts.
func scopeRequest(req request) request {
	req.Directory, _ = os.Getwd()
	req.ID = requestID(req)
	return req
}

// Lets the approver see which PR a comment, merge, or review is for. Empty
// when the request names its own title or the lookup fails. pr edit reuses
// the title its review already fetched.
func currentTitle(gh string, req request, fetched string) string {
	if req.HasTitle || req.Target == "" {
		return ""
	}
	if req.Operation == "edit" {
		return fetched
	}
	args := []string{"pr", "view", req.Target, "--json", "title", "--jq", ".title"}
	if req.Repo != "" {
		args = append(args, "--repo", req.Repo)
	}
	return ghLookup(gh, req.Directory, args...)
}

// Read-only and short, because the request lock is held while it runs.
// Failure gives an empty result.
func ghLookup(gh, directory string, args ...string) string {
	ctx, cancel := context.WithTimeout(context.Background(), fetchTimeout)
	defer cancel()
	cmd := exec.CommandContext(ctx, gh, args...)
	cmd.Dir = directory
	output, err := cmd.Output()
	if err != nil {
		return ""
	}
	return strings.TrimSpace(string(output))
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

func reportOrRun(id, gh string) int {
	return reportOrRunOutput(id, gh, os.Stdout)
}

// The lock stays held while gh runs. A running record found under the lock
// therefore means that run died, and it is never rerun. gh runs an approved
// request: the foreground writer passes the gh it found on PATH, and the
// runner passes rec.GH because its PATH may lack gh.
func reportOrRunOutput(id, gh string, output io.Writer) int {
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
		return runGH(rec, gh, output)
	case stateRan:
		fmt.Fprintf(stderr, "pde-gh-write: already ran %s, exit %d\n", id, rec.Exit)
		fmt.Fprint(output, rec.Output)
		return rec.Exit
	case stateDeclined:
		fmt.Fprintf(stderr, "pde-gh-write: declined %s\n", id)
		return exitDeclined
	case stateRunning:
		fmt.Fprintf(stderr, "pde-gh-write: interrupted %s\n", id)
		return exitInterrupted
	}
	fmt.Fprintf(stderr, "pde-gh-write: still waiting for approval of %s; it runs once approved, so rerun this exact command later to read the result\n", id)
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

// Keeps a noisy gh command from bloating the record that reruns print.
const maxOutput = 64 << 10

// Keeps the first maxOutput bytes and accepts the rest unwritten, so gh never
// sees a short write.
type limitedBuffer struct{ bytes.Buffer }

func (b *limitedBuffer) Write(p []byte) (int, error) {
	kept := p[:min(len(p), max(0, maxOutput-b.Len()))]
	_, _ = b.Buffer.Write(kept)
	return len(p), nil
}

func runGH(rec record, gh string, output io.Writer) int {
	if gh == "" || !isExecutable(gh) {
		// The request stays approved, so rerunning the command runs it.
		fmt.Fprintf(stderr, "pde-gh-write: cannot find gh to run approved %s; rerun this exact command\n", rec.ID)
		return 1
	}
	rec.State = stateRunning
	if err := save(rec); err != nil {
		return printError(err)
	}
	capture := &limitedBuffer{}
	cmd := exec.Command(gh, rec.Args...)
	cmd.Dir = rec.Directory
	cmd.Stdout, cmd.Stderr = capture, capture
	if rec.BodyFile {
		cmd.Stdin = strings.NewReader(rec.Body)
	}
	// Lets this one approved command through the gh guard.
	cmd.Env = append(os.Environ(), "PDE_GH_WRITE=1")
	rec.Exit = 0
	err := cmd.Run()
	rec.Output = capture.String()
	if err != nil {
		var exitErr *exec.ExitError
		rec.Exit = 1
		if errors.As(err, &exitErr) {
			rec.Exit = exitErr.ExitCode()
		} else {
			rec.Output += fmt.Sprintf("pde-gh-write: %v\n", err)
		}
	}
	rec.State = stateRan
	if err := save(rec); err != nil {
		// The record stays running, so reruns report interrupted and never rerun.
		fmt.Fprintf(stderr, "pde-gh-write: gh exited %d but the result was not saved: %v\n", rec.Exit, err)
	}
	fmt.Fprint(output, rec.Output)
	return rec.Exit
}
