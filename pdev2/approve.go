package main

import (
	"errors"
	"fmt"
	"io"
	"os"
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
// loaded. It clears the phone question's PID and starts the runner on
// approval. saveAnswer then ends the phone question; moshiAsk must not,
// because that question is the caller.
func recordAnswer(rec record, choice string) (record, error) {
	rec.MoshiPID = 0
	rec.State = stateDeclined
	if choice == "yes" {
		rec.State = stateApproved
	}
	if err := save(rec); err != nil {
		return rec, err
	}
	if rec.State == stateApproved {
		rec = ensureRunner(rec)
	}
	return rec, nil
}

func approvePopup(id string) int {
	if id == "" {
		return 1
	}
	rec, err := load(id)
	if err != nil || rec.State != statePending || expired(rec) {
		return 1
	}
	choice := popupAnswer(rec)
	if choice == "" {
		return 0
	}
	return saveAnswer(id, choice)
}

// A terminal error closes the popup without an answer; the request keeps
// waiting for one on the phone or in a terminal.
func popupAnswer(rec record) string {
	choice, err := askPopup(os.Stdin, os.Stdout, rec)
	if err != nil {
		return ""
	}
	return choice
}
