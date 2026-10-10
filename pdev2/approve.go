package main

import (
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"time"
)

func approve(args []string) int {
	if len(args) != 1 {
		fmt.Fprintln(stderr, "usage: pde-pr-approve REQUEST_ID")
		return exitRejected
	}
	rec, err := load(args[0])
	if err != nil || rec.State != statePending || expired(rec) {
		fmt.Fprintf(stderr, "pde-pr-approve: no pending request %s\n", args[0])
		return exitFailed
	}
	// Only /dev/tty can answer; redirected agent input cannot approve.
	tty, err := os.OpenFile("/dev/tty", os.O_RDWR, 0)
	if err != nil {
		fmt.Fprintf(stderr, "pde-pr-approve: cannot open the terminal: %v\n", err)
		return exitFailed
	}
	defer tty.Close()
	if err := showPager(tty, rec.ApprovalText); err != nil {
		fmt.Fprintf(stderr, "pde-pr-approve: less failed: %v\n", err)
		return exitFailed
	}
	choice, err := askYesNo(tty)
	if errors.Is(err, io.EOF) {
		// Ctrl-D ends input without an answer; record nothing.
		return 0
	}
	if err != nil {
		fmt.Fprintf(stderr, "pde-pr-approve: cannot read the answer: %v\n", err)
		return exitFailed
	}
	return saveAnswer(rec.ID, choice)
}

func saveAnswer(id, choice string) int {
	lockFile, err := lockRequest(id)
	if err != nil {
		fmt.Fprintf(stderr, "pde-pr-approve: %v\n", err)
		return exitFailed
	}
	defer lockFile.Close()
	// Checked again under the lock, because another approver may have answered
	// or the request may have expired while the pager was open.
	rec, err := load(id)
	if err != nil || rec.State != statePending || expired(rec) {
		fmt.Fprintf(stderr, "pde-pr-approve: request %s was already handled or expired\n", id)
		return 0
	}
	ask := rec.MoshiPID
	if _, err := recordAnswer(rec, choice); err != nil {
		fmt.Fprintf(stderr, "pde-pr-approve: %v\n", err)
		return exitFailed
	}
	stopMoshi(ask)
	return 0
}

// recordAnswer saves choice on a pending record that the caller has locked and
// loaded. It clears the phone ask's PID, starts the runner on approval, and
// clears the pane marker. saveAnswer then stops the phone ask; moshiAsk must
// not, because that ask is the caller.
func recordAnswer(rec record, choice string) (record, error) {
	rec.MoshiPID = 0
	rec.State = stateDeclined
	if choice == choiceYes {
		rec.State = stateApproved
	}
	if err := save(rec); err != nil {
		return rec, err
	}
	if rec.State == stateApproved {
		rec = ensureRunner(rec)
	}
	clearRecordMarker(rec)
	return rec, nil
}

// Answering or dismissing moves on to the oldest request still waiting, so one
// popup can work through several requests.
func approvePopup(id string) int {
	if id == "" {
		return 1
	}
	for {
		rec, err := load(id)
		if err != nil || rec.State != statePending || expired(rec) {
			return 1
		}
		switch choice := popupAnswer(rec, queuePosition(id), popupSettle); choice {
		case choiceYes, choiceNo:
			if code := saveAnswer(id, choice); code != 0 {
				return code
			}
		case choiceDismiss:
			dismissRequest(id)
		case choiceFocus:
			focusRequest(rec)
			return 0
		default:
			// Closing the popup is not an answer.
			return 0
		}
		next := queuedRecords()
		if len(next) == 0 {
			return 0
		}
		id = next[0].ID
	}
}

// Counts only requests the popup will visit; a dismissed request opened from
// the inbox counts as first.
func queuePosition(id string) string {
	queue := queuedRecords()
	for i, rec := range queue {
		if rec.ID == id {
			return fmt.Sprintf("%d of %d", i+1, len(queue))
		}
	}
	return fmt.Sprintf("1 of %d", len(queue)+1)
}

// A dismissed request stays pending but no longer opens on its own; the inbox
// still lists it.
func dismissRequest(id string) {
	lockFile, err := lockRequest(id)
	if err != nil {
		return
	}
	defer lockFile.Close()
	rec, err := load(id)
	if err != nil || rec.State != statePending {
		return
	}
	rec.Dismissed = true
	if save(rec) == nil {
		clearRecordMarker(rec)
	}
}

func focusRequest(rec record) {
	pane := findAgent(rec.SessionID).Pane
	if pane == "" {
		pane = rec.PaneID
	}
	if pane != "" {
		_ = exec.Command("herdr", "agent", "focus", pane).Run()
	}
}

// Tests point these at a pseudo-terminal.
var popupIn, popupOut = os.Stdin, os.Stdout

func popupAnswer(rec record, queue string, settle time.Duration) string {
	choice, err := askPopup(popupIn, popupOut, rec, queue, settle)
	if err != nil {
		return ""
	}
	return choice
}

// Opens the popup on the oldest pending request, including dismissed ones.
func openInbox() int {
	pending := inboxRecords()
	if len(pending) == 0 {
		// The key otherwise seems to do nothing.
		_ = exec.Command("herdr", "notification", "show", "PR approval", "--body", "No approvals waiting").Run()
		return 0
	}
	if err := openApproval(pending[0].ID); err != nil {
		return 1
	}
	return 0
}
