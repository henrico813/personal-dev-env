package cmd

import (
	"fmt"
	"os"
	"os/exec"
	"time"

	"pdev2/internal/approval"
	"pdev2/internal/terminal"
)

// Answering or dismissing moves on to the oldest request still waiting, so one
// popup can work through several requests.
func approvePopup(id string) int {
	if id == "" {
		return 1
	}
	rec, err := approval.Load(id)
	if err != nil || rec.State != approval.StatePending || approval.Expired(rec) {
		return 1
	}
	for {
		switch choice := popupAnswer(rec, queuePosition(rec.ID), terminal.PopupSettle); choice {
		case approval.ChoiceYes, approval.ChoiceNo:
			if code := saveAnswer(rec.ID, choice); code != 0 {
				return code
			}
		case approval.ChoiceDismiss:
			dismissRequest(rec.ID)
		case approval.ChoiceFocus:
			focusRequest(rec)
			return 0
		default:
			// Closing the popup is not an answer.
			return 0
		}
		next := approval.Queued()
		if len(next) == 0 {
			return 0
		}
		rec = next[0]
	}
}

// Counts only requests the popup will visit; a dismissed request opened from
// the inbox counts as first.
func queuePosition(id string) string {
	queue := approval.Queued()
	for i, rec := range queue {
		if rec.ID == id {
			return fmt.Sprintf("%d of %d", i+1, len(queue))
		}
	}
	return fmt.Sprintf("1 of %d", len(queue)+1)
}

// A dismissed request stays pending but no longer opens on its own; the inbox
// still lists it. Dismissing is not an answer, so it keeps the request's age;
// otherwise dismissing and reopening it could keep it waiting past four hours.
func dismissRequest(id string) {
	lockFile, err := approval.Lock(id)
	if err != nil {
		return
	}
	defer lockFile.Close()
	rec, err := approval.Load(id)
	if err != nil || rec.State != approval.StatePending {
		return
	}
	rec.Dismissed = true
	if approval.SaveKeepingAge(rec) == nil {
		clearRecordMarker(rec)
	}
}

// Tests point these at a pseudo-terminal.
var popupIn, popupOut = os.Stdin, os.Stdout

// A terminal error closes the popup without an answer; the request keeps
// waiting for one on the phone or in a terminal.
func popupAnswer(rec approval.Record, queue string, settle time.Duration) string {
	choice, err := terminal.AskPopup(popupIn, popupOut, rec, queue, settle)
	if err != nil {
		return ""
	}
	return choice
}

// Opens the popup on the oldest pending request, including dismissed ones.
func openInbox() int {
	pending := approval.Inbox()
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
