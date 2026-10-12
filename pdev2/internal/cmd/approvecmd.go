package cmd

import (
	"errors"
	"fmt"
	"io"
	"os"

	"pdev2/internal/approval"
	"pdev2/internal/terminal"
)

func approve(args []string) int {
	if len(args) != 1 {
		fmt.Fprintln(stderr, "usage: pde-pr-approve REQUEST_ID")
		return exitRejected
	}
	rec, err := approval.Load(args[0])
	if err != nil || rec.State != approval.StatePending || approval.Expired(rec) {
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
	if err := terminal.ShowPager(tty, rec.ApprovalText); err != nil {
		fmt.Fprintf(stderr, "pde-pr-approve: less failed: %v\n", err)
		return exitFailed
	}
	choice, err := terminal.AskYesNo(tty)
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
