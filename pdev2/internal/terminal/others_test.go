package terminal

import (
	"os"
	"syscall"
	"testing"

	"pdev2/internal/approvaltest"
)

// With nothing waiting here, Ctrl+b a used to say "No approvals waiting"
// while a request waited on another machine, and merges stayed blocked. The
// check takes seconds, so the screen must show its result when it arrives,
// and a tap must close it, because on the phone a tap is often the only
// input.
func TestMachinesScreenUpdatesThenTapCloses(t *testing.T) {
	// Run the screen on a pseudo-terminal at Herdr's size; see
	// TestPopupDiscardsEarlyInput.
	master, terminal := approvaltest.OpenPTY(t)
	approvaltest.SetPopupSize(t, terminal, 37, 14)
	release := make(chan struct{})
	check := func() []string {
		<-release
		return []string{"Wallace: 1 waiting in", "  notes", "Lab: not reachable"}
	}
	closed := make(chan error, 1)
	go func() { closed <- ShowOtherMachines(approvaltest.PopupTerminal(t, terminal), terminal, check) }()
	approvaltest.ReadUntil(t, master, "Checking other machines...")

	close(release)
	approvaltest.ShownBeforeAnswer(t, master, "Lab: not reachable", closed)
	// A left-button press and release anywhere on the screen.
	approvaltest.WriteTerminal(t, master, "\x1b[<0;5;5M\x1b[<0;5;5m")

	if err := approvaltest.Receive(t, closed); err != nil {
		t.Fatal(err)
	}
}

// Herdr shrinks the popup to fit a smaller client, such as a phone, and
// grows it back. Without a redraw, lines stay cut at the old width.
func TestMachinesScreenRedrawsWhenResized(t *testing.T) {
	// Start narrow, so the machine line is cut, and wait for the result.
	master, terminal := approvaltest.OpenPTY(t)
	approvaltest.SetPopupSize(t, terminal, 20, 14)
	check := func() []string { return []string{"Wallace: 1 waiting in", "  notes"} }
	closed := make(chan error, 1)
	go func() { closed <- ShowOtherMachines(approvaltest.PopupTerminal(t, terminal), terminal, check) }()
	approvaltest.ReadUntil(t, master, "Wallace: 1 waiting i")

	// Grow the terminal and send the resize signal, as a terminal does. The
	// screen runs in this process, so the signal goes to the test itself.
	approvaltest.SetPopupSize(t, terminal, 37, 14)
	if err := syscall.Kill(os.Getpid(), syscall.SIGWINCH); err != nil {
		t.Fatal(err)
	}

	approvaltest.ShownBeforeAnswer(t, master, "Wallace: 1 waiting in", closed)
	approvaltest.WriteTerminal(t, master, "q")
	approvaltest.Receive(t, closed)
}
