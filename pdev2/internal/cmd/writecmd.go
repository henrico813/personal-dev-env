package cmd

import (
	"errors"
	"fmt"
	"io"
	"os"
	"time"

	"pdev2/internal/approval"
)

// Agent commands are killed after about two minutes, so the wait must end first.
var answerTimeout = 90 * time.Second

func requestWrite(args []string) int {
	req, err := approval.Parse(args, os.Stdin)
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
	if state == approval.StatePending {
		return waitForAnswer(req.ID, gh)
	}
	return reportOrRun(req.ID, gh)
}

// The lock stops two identical first requests from both creating a record;
// the later save could overwrite an approval or a finished run. A new request
// also starts the Herdr notification, the phone question, and the runner; a
// pending one restarts a phone question or runner that died, and opens the
// popup again if it was dismissed.
func loadOrCreate(req approval.Request, gh string) (string, error) {
	lockFile, err := approval.Lock(req.ID)
	if err != nil {
		return "", err
	}
	defer lockFile.Close()
	rec, err := approval.Load(req.ID)
	// An expired record of any state is replaced, so after four hours the same
	// command needs a new approval and may run again.
	if err == nil && !approval.Expired(rec) {
		if rec.State == approval.StatePending {
			// The owner asks the agent to send a dismissed request again, and a
			// rerun is all the agent can do. Saved here, because startMoshi and
			// ensureRunner save rec only when they restart something.
			if rec.Dismissed {
				rec.Dismissed = false
				// Like dismissing, this keeps the request's age.
				if err := approval.SaveKeepingAge(rec); err != nil {
					return "", err
				}
				fmt.Fprintf(stderr, "pde-gh-write: %s had been dismissed in the Herdr popup; it is back in the popup queue\n", rec.ID)
				go notifyRequest(rec)
			}
			// Replace a phone question that ended without an answer, or the
			// request would wait with nothing to answer on the phone. startMoshi
			// does nothing while the earlier question is still running.
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
	rec = approval.Record{ID: req.ID, Args: req.Args, Title: req.Title, Body: req.Body,
		BodyFile: req.BodyFile, ApprovalText: text, State: approval.StatePending,
		Created: time.Now(), Directory: req.Directory, Repo: req.Repo, GH: gh,
		PRTitle: currentTitle(gh, req, fetchedTitle), SessionID: os.Getenv("OPENCODE_SESSION_ID")}
	// Filled in before the first save, because a later save outside this lock
	// could overwrite an answer.
	rec.PaneID, rec.Workspace, rec.SessionTitle = agentContext(rec.SessionID)
	if err := approval.Save(rec); err != nil {
		return "", err
	}
	fmt.Fprintf(stderr, "pde-gh-write: waiting for approval in the Herdr popup or on the phone, or run pde-pr-approve %s in a terminal; it runs once approved, so rerun this exact command to read the result\n", req.ID)
	// Opening the popup runs in the background so a slow Herdr cannot delay the
	// wait for an answer. It may not finish if this program exits first.
	go notifyRequest(rec)
	// runnerStart saves rec, so it needs the ask PID that startMoshi saved.
	rec = startMoshi(rec)
	runnerStart(rec)
	return approval.StatePending, nil
}

// The same command in another directory is a different request, so an
// approval never carries over between checkouts.
func scopeRequest(req approval.Request) approval.Request {
	req.Directory, _ = os.Getwd()
	req.ID = approval.RequestID(req)
	return req
}

func waitForAnswer(id, gh string) int {
	deadline := time.Now().Add(answerTimeout)
	for time.Now().Before(deadline) {
		if rec, err := approval.Load(id); err == nil && rec.State != approval.StatePending {
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
	lockFile, err := approval.Lock(id)
	if err != nil {
		return printError(err)
	}
	defer lockFile.Close()
	rec, err := approval.Load(id)
	if err != nil {
		return printError(err)
	}
	switch rec.State {
	case approval.StateApproved:
		return runGH(rec, gh, output)
	case approval.StateRan:
		fmt.Fprintf(stderr, "pde-gh-write: already ran %s, exit %d\n", id, rec.Exit)
		fmt.Fprint(output, rec.Output)
		return rec.Exit
	case approval.StateDeclined:
		fmt.Fprintf(stderr, "pde-gh-write: declined %s\n", id)
		return exitDeclined
	case approval.StateRunning:
		fmt.Fprintf(stderr, "pde-gh-write: interrupted %s\n", id)
		return exitInterrupted
	}
	fmt.Fprintf(stderr, "pde-gh-write: still waiting for approval of %s; it runs once approved, so rerun this exact command later to read the result\n", id)
	// The phone question runs detached, so this is where its failure is seen.
	if rec.MoshiError != "" {
		fmt.Fprintf(stderr, "pde-gh-write: the phone question failed (%s); ask the user to check moshi-hook with moshi-hook status and moshi-hook doctor\n", rec.MoshiError)
	}
	return exitWaiting
}
