package cmd

import (
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"time"

	"pdev2/internal/approval"
	"pdev2/internal/terminal"
)

// The inbox sets this for a popup with no request, to show other machines.
const checkMachinesEnv = "PDE_CHECK_MACHINES"

// Answering or dismissing moves on to the oldest request still waiting, so one
// popup can work through several requests. Without id, when the inbox sets
// PDE_CHECK_MACHINES, it shows where approvals wait on other machines.
func approvePopup(id string) int {
	if id == "" {
		if os.Getenv(checkMachinesEnv) != "1" {
			return 1
		}
		if terminal.ShowOtherMachines(popupIn, popupOut, otherMachineLines) != nil {
			return 1
		}
		return 0
	}
	rec, err := approval.Load(id)
	if err != nil || rec.State != approval.StatePending || approval.Expired(rec) {
		return 1
	}
	// Herdr refuses every other popup while this one is open and cannot say
	// where it is, so save its workspace for requests that cannot open their
	// own. Herdr gives a popup no HERDR_WORKSPACE_ID; the workspace it opened
	// in is workspace_id in HERDR_PLUGIN_CONTEXT_JSON.
	var context struct {
		Workspace string `json:"workspace_id"`
	}
	if json.Unmarshal([]byte(os.Getenv("HERDR_PLUGIN_CONTEXT_JSON")), &context) == nil && context.Workspace != "" {
		if approval.SavePopup(approval.Popup{PID: os.Getpid(), Workspace: context.Workspace}) == nil {
			defer approval.RemovePopup()
		}
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
// With none here and a saved Herdr machine, the popup says where requests
// wait on other machines; those are answered there.
// When the popup does not open, stderr and a notification say why and the
// inbox exits 1; if an approval popup is already open, it first moves to
// that popup's workspace, because the user asked to see approvals.
func openInbox() int {
	pending := approval.Inbox()
	var err error
	switch {
	case len(pending) > 0:
		err = openApproval(pending[0].ID)
	case len(savedMachines()) > 0:
		err = openPopup("inbox", map[string]string{checkMachinesEnv: "1"})
	default:
		// The key otherwise seems to do nothing.
		runInboxHerdr("notification", "show", "PR approval", "--body", "No approvals waiting")
		return 0
	}
	if err == nil {
		return 0
	}
	// Herdr keeps stderr in herdr plugin log list, so the reason shows even
	// when herdr cannot be run to show the notification.
	fmt.Fprintf(stderr, "pde-pr-approve: cannot open the approval popup: %v\n", err)
	body, workspace := popupFailure(err)
	if workspace != "" {
		runInboxHerdr("workspace", "focus", workspace)
	}
	runInboxHerdr("notification", "show", "PR approval", "--body", body)
	return 1
}

// runInboxHerdr runs a herdr command, such as notification show, and
// writes its failure to stderr.
func runInboxHerdr(args ...string) {
	if err := exec.Command(herdrPath(), args...).Run(); err != nil {
		fmt.Fprintf(stderr, "pde-pr-approve: herdr %s %s: %v\n", args[0], args[1], err)
	}
}
