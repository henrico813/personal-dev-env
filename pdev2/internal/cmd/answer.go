package cmd

import (
	"fmt"

	"pdev2/internal/approval"
)

func saveAnswer(id, choice string) int {
	lockFile, err := approval.Lock(id)
	if err != nil {
		fmt.Fprintf(stderr, "pde-pr-approve: %v\n", err)
		return exitFailed
	}
	defer lockFile.Close()
	// Checked again under the lock, because another approver may have answered
	// or the request may have expired while the pager was open.
	rec, err := approval.Load(id)
	if err != nil || rec.State != approval.StatePending || approval.Expired(rec) {
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
// loaded. It clears the phone question's PID, starts the runner on approval,
// and clears the pane marker. saveAnswer then ends the phone question;
// moshiAsk must not, because that question is the caller.
func recordAnswer(rec approval.Record, choice string) (approval.Record, error) {
	rec.MoshiPID = 0
	rec.State = approval.StateDeclined
	if choice == approval.ChoiceYes {
		rec.State = approval.StateApproved
	}
	if err := approval.Save(rec); err != nil {
		return rec, err
	}
	if rec.State == approval.StateApproved {
		rec = ensureRunner(rec)
	}
	clearRecordMarker(rec)
	return rec, nil
}
