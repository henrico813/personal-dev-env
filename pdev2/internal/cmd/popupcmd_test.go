package cmd

import (
	"fmt"
	"os"
	"testing"
	"time"

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
	"pdev2/internal/terminal"
)

// On a phone, an impatient second tap lands on the next request, which has
// just appeared under the finger, and would approve it unread. Each queued
// request therefore opens with a settle pause and no selected button.
// TestPopupAppliesTapAndSettleRules checks that taps during the pause are
// ignored; this test checks that the queue applies the pause and the reset
// to every request, not only the first.
func TestQueuedPopupsSettleAndStartUnselected(t *testing.T) {
	master, layout := queueTwoMerges(t)
	press := fmt.Sprintf("\x1b[<0;10;%dM", layout.ApproveTop)
	// Run the queue in the background; approvePopup returns once no
	// undismissed request is left.
	done := make(chan int, 1)
	go func() { done <- approvePopup("first") }()

	// The first popup settles too; wait for the redraw that clears its notice.
	approvaltest.ReadUntil(t, master, "Typed:")
	approvaltest.ShownBeforeAnswer(t, master, "Typed:", done)
	// Approve the first request with a selecting tap and a confirming tap.
	approvaltest.WriteTerminal(t, master, press)
	approvaltest.ShownBeforeAnswer(t, master, "tap APPROVE again to confirm", done)
	approvaltest.WriteTerminal(t, master, press)
	// The second request opens with the pause notice. Wait for the redraw that
	// clears it, so the taps below land after the pause.
	approvaltest.ReadUntil(t, master, terminal.SettleNotice)
	approvaltest.ShownBeforeAnswer(t, master, "Typed:", done)
	if second, err := approval.Load("second"); err != nil || second.State != approval.StatePending {
		t.Fatalf("second request = %#v, %v; want pending", second, err)
	}
	// The first request's confirmed selection must not carry over, so one tap
	// only selects.
	approvaltest.WriteTerminal(t, master, press)
	approvaltest.ShownBeforeAnswer(t, master, "tap APPROVE again to confirm", done)
	approvaltest.WriteTerminal(t, master, press)

	select {
	case <-done:
	case <-time.After(5 * time.Second):
		t.Fatal("a tap after the settle time was ignored")
	}
	if second, err := approval.Load("second"); err != nil || second.State != approval.StateApproved {
		t.Fatalf("second request = %#v, %v; want approved", second, err)
	}
}

// queueTwoMerges saves two pending merges, oldest first, and points the popup
// at a new pseudo-terminal. It returns the terminal's master side and the
// first request's button layout.
func queueTwoMerges(t *testing.T) (*os.File, terminal.Layout) {
	t.Helper()
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	now := time.Now()
	for i, id := range []string{"first", "second"} {
		rec := approval.Record{ID: id, Args: []string{"pr", "merge", fmt.Sprint(i + 1)}, State: approval.StatePending, Created: now.Add(time.Duration(i) * time.Second)}
		if err := approval.Save(rec); err != nil {
			t.Fatal(err)
		}
	}
	master, tty := approvaltest.OpenPTY(t)
	approvaltest.SetPopupSize(t, tty, 37, 14)
	approvaltest.Replace(t, &popupIn, approvaltest.PopupTerminal(t, tty))
	approvaltest.Replace(t, &popupOut, tty)
	_, layout := terminal.PopupScreen(terminal.View{Rec: approval.Record{Args: []string{"pr", "merge", "1"}}, Queue: "1 of 2"}, 37, 14)
	return master, layout
}

// Herdr shows one popup at a time, and a popup left open in a workspace the
// user is not viewing hid every later request: Herdr refused their popups
// and could not say where the open one was. The popup must save its
// workspace while it is open, and only then, or the inbox cannot lead the
// user to it.
func TestPopupRecordsItsWorkspaceWhileOpen(t *testing.T) {
	master, _ := queueTwoMerges(t)
	// Herdr passes the workspace only in the plugin context, as in this
	// trimmed copy from a real popup.
	t.Setenv("HERDR_PLUGIN_CONTEXT_JSON", `{"workspace_id":"w1G","workspace_label":"home-hermes-stash","tab_id":"w1G:t1","focused_pane_id":"w1G:p1","invocation_source":"api","correlation_id":"plugin-pane"}`)
	done := make(chan int, 1)
	go func() { done <- approvePopup("first") }()

	// The popup saves its workspace before it draws.
	approvaltest.ReadUntil(t, master, "Typed:")
	open, openErr := approval.LoadPopup()
	// g closes the popup at once.
	approvaltest.WriteTerminal(t, master, "g")
	approvaltest.Receive(t, done)
	_, closedErr := approval.LoadPopup()

	if openErr != nil || open.PID != os.Getpid() || open.Workspace != "w1G" {
		t.Fatalf("while open: popup = %#v, %v", open, openErr)
	}
	if closedErr == nil {
		t.Fatal("popup location kept after the popup closed")
	}
}

// Dismissing must stop a request from reopening in the popup queue while
// the inbox can still find it. Dismissing first used the save that records an
// answer, which restarted the four-hour expiry, so a request dismissed and
// reopened from the inbox could keep waiting indefinitely.
func TestDismissedRequestStaysHidden(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The request was last changed an hour ago.
	saved := time.Now().Add(-time.Hour).Round(0)
	rec := approval.Record{ID: "dismissed", State: approval.StatePending, Updated: saved}
	if err := approval.Write(rec); err != nil {
		t.Fatal(err)
	}

	dismissRequest(rec.ID)

	if got := approval.Queued(); len(got) != 0 {
		t.Fatalf("visible pending = %d", len(got))
	}
	got := approval.Inbox()
	if len(got) != 1 || got[0].ID != rec.ID {
		t.Fatalf("inbox pending = %#v", got)
	}
	if !got[0].Updated.Equal(saved) {
		t.Errorf("dismissing moved the last change from %v to %v", saved, got[0].Updated)
	}
}
