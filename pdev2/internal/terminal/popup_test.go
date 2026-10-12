package terminal

import (
	"fmt"
	"os"
	"slices"
	"strings"
	"syscall"
	"testing"
	"unicode/utf8"

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
)

// A tap answers only on the rows of the APPROVE and DECLINE buttons, so the
// rows the screen draws and the rows a tap is checked against must agree, and
// every line must fit. In live testing Herdr 0.9.1 gave the popup 37x14 inside
// its 40x16 border, where buttons are two rows tall. A shorter popup gets
// one-row buttons, where a one-row mistake would make a tap on a button miss.
func TestPopupScreenFitsHerdrPane(t *testing.T) {
	rec := approval.Record{ID: "123456789", ApprovalText: strings.Repeat("request text that must be clipped ", 4)}
	for _, size := range []struct{ cols, rows int }{{37, 14}, {40, 16}, {37, 10}} {
		lines, layout := PopupScreen(View{Rec: rec, Queue: "1 of 1"}, size.cols, size.rows)

		if len(lines) != size.rows {
			t.Fatalf("%dx%d rendered %d rows", size.cols, size.rows, len(lines))
		}
		for _, line := range lines {
			if len(line) > size.cols {
				t.Fatalf("%dx%d line exceeded columns: %q", size.cols, size.rows, line)
			}
		}
		if strings.Count(strings.Join(lines, "\n"), "v: full; g: focus; q/Esc: dismiss") != 1 {
			t.Fatal("dismiss hint was not shown once")
		}
		for label, want := range map[string]string{"[ APPROVE ]": "yes", "[ DECLINE ]": "no"} {
			row := slices.IndexFunc(lines, func(line string) bool { return strings.Contains(line, label) })
			if row < 0 {
				t.Fatalf("%dx%d did not draw %s", size.cols, size.rows, label)
			}
			if got := layout.hit(1, row+1); got != want {
				t.Errorf("%dx%d tap on %s at row %d = %q, want %q", size.cols, size.rows, label, row+1, got, want)
			}
		}
	}
}

// The approver decides on the text that will be posted. In live testing the
// 37x14 popup first showed the ID, time, and command lines of the full
// approval text instead of the title and body. Later the context lines
// (repository, Herdr session, PR title) filled its three request rows, so a
// comment's body and an edit's changed lines were not shown at all. With
// two-row buttons, the six rows below the first line hold the three context
// lines and three lines of the body.
func TestPopupShowsPostedTextBesideContext(t *testing.T) {
	context := approval.Record{Repo: "octo/app", Workspace: "app", SessionTitle: "review login fix", PRTitle: "Fix login timeout",
		ApprovalText: "id: long\ntime: long\ncommand: long\nbody: long"}
	comment := context
	comment.Args = []string{"pr", "comment", "12"}
	comment.Body = "first body line\nsecond body line\nthird body line"
	edit := context
	edit.Args = []string{"pr", "edit", "12", "--title", "New title"}
	edit.Title = "New title"
	edit.ApprovalText = approval.EditHeader + "Title:\n-Old title\n+New title\n"
	cases := []struct {
		name string
		rec  approval.Record
		want []string
	}{
		{name: "comment", rec: comment, want: []string{"comment #12", "octo/app", "app / review login fix", "Fix login timeout",
			"first body line", "second body line", "third body line"}},
		{name: "edit", rec: edit, want: []string{"edit #12", "octo/app", "app / review login fix", "New title", "-Old title", "+New title"}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			lines, _ := PopupScreen(View{Rec: tc.rec, Queue: "1 of 1"}, 37, 14)

			rendered := strings.Join(lines, "\n")
			for _, want := range tc.want {
				if !strings.Contains(rendered, want) {
					t.Fatalf("popup missing %q:\n%s", want, rendered)
				}
			}
		})
	}
}

// Titles can contain characters such as é that take more than one byte.
// Cutting a long title by bytes split such a character and drew invalid text,
// so lines are cut by character.
func TestPopupClipsWithoutSplittingCharacters(t *testing.T) {
	rec := approval.Record{ID: "id", Args: []string{"pr", "create"}, Title: strings.Repeat("é", 60)}

	lines, _ := PopupScreen(View{Rec: rec, Queue: "1 of 1"}, 37, 14)

	for _, line := range lines {
		if !utf8.ValidString(line) || utf8.RuneCountInString(line) > 37 {
			t.Fatalf("line %q is invalid or wider than 37 characters", line)
		}
	}
	if want := strings.Repeat("é", 37); !slices.Contains(lines, want) {
		t.Fatalf("lines %q do not contain the clipped title", lines)
	}
}

// The agent writes the title and body. If the terminal obeyed control
// characters in them, such as ESC [2J (clear the screen), an agent could wipe
// the popup or draw a different request over it. They must appear as text.
func TestPopupEscapesControlText(t *testing.T) {
	rec := approval.Record{ID: "id", Body: "title\x1b[2J\rbody\u009bend"}

	lines, _ := PopupScreen(View{Rec: rec, Queue: "1 of 1"}, 40, 16)

	rendered := strings.Join(lines, "\n")
	if strings.ContainsAny(rendered, "\x1b\r\u009b") || !strings.Contains(rendered, "^[") || !strings.Contains(rendered, "^M") || !strings.Contains(rendered, "\\u009b") {
		t.Fatalf("rendered = %q", rendered)
	}
}

// The program asks Herdr to put the keyboard on the popup when it opens, so a
// yes and Enter typed for another terminal just before could land in it and
// approve. Input typed before the popup is drawn must be thrown away.
func TestPopupDiscardsEarlyInput(t *testing.T) {
	// A pseudo-terminal is a fake terminal: the popup reads keys from and draws
	// on terminal, and the test types into and reads the screen from master.
	master, terminal := approvaltest.OpenPTY(t)
	approvaltest.SetPopupSize(t, terminal, 37, 14)
	// Type yes before the popup starts; it must be discarded.
	if _, err := master.WriteString("yes\n"); err != nil {
		t.Fatal(err)
	}
	// Start the popup in the background; it blocks until it gets an answer.
	result := make(chan string, 1)
	go func() {
		choice, _ := AskPopup(approvaltest.PopupTerminal(t, terminal), terminal, approval.Record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
		result <- choice
	}()
	// Wait for the draw, so the answer below arrives after the early input is
	// thrown away.
	approvaltest.ReadUntil(t, master, "Typed:")

	if _, err := master.WriteString("no\n"); err != nil {
		t.Fatal(err)
	}

	if got := approvaltest.Receive(t, result); got != "no" {
		t.Fatalf("answer %q", got)
	}
}

// q and Esc must close the popup and leave the request waiting. In live
// testing both keys were ignored, so users could not close the popup without
// approving or declining.
func TestPopupDismissesWithoutAnswer(t *testing.T) {
	for _, dismiss := range []string{"q", "\x1b"} {
		t.Run(dismiss, func(t *testing.T) {
			// Run the popup on a pseudo-terminal at Herdr's size; see
			// TestPopupDiscardsEarlyInput.
			master, terminal := approvaltest.OpenPTY(t)
			approvaltest.SetPopupSize(t, terminal, 37, 14)
			result := make(chan string, 1)
			go func() {
				choice, _ := AskPopup(approvaltest.PopupTerminal(t, terminal), terminal, approval.Record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
				result <- choice
			}()
			// Wait for the draw; a key sent earlier would be discarded as early input.
			approvaltest.ReadUntil(t, master, "Typed:")

			if _, err := master.WriteString(dismiss); err != nil {
				t.Fatal(err)
			}

			if got := approvaltest.Receive(t, result); got != approval.ChoiceDismiss {
				t.Fatalf("answer %q", got)
			}
		})
	}
}

// Approving takes the whole word yes, because an earlier version approved on
// one stray key. In live testing a user typed y and Enter several times and
// saw nothing happen; the popup must refuse y and say what to type.
func TestPopupRejectsLoneY(t *testing.T) {
	// Run the popup on a pseudo-terminal at Herdr's size; see
	// TestPopupDiscardsEarlyInput.
	master, terminal := approvaltest.OpenPTY(t)
	approvaltest.SetPopupSize(t, terminal, 37, 14)
	result := make(chan string, 1)
	go func() {
		choice, _ := AskPopup(approvaltest.PopupTerminal(t, terminal), terminal, approval.Record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
		result <- choice
	}()
	// Wait for the draw; a key sent earlier would be discarded as early input.
	approvaltest.ReadUntil(t, master, "Typed:")

	if _, err := master.WriteString("y\n"); err != nil {
		t.Fatal(err)
	}

	// The hint proves y was refused without an answer; q then ends the popup
	// so the test can check that nothing was chosen.
	approvaltest.ReadUntil(t, master, "Type yes or no, then Enter")
	if _, err := master.WriteString("q"); err != nil {
		t.Fatal(err)
	}
	if got := approvaltest.Receive(t, result); got != approval.ChoiceDismiss {
		t.Fatalf("answer %q", got)
	}
}

// Tapping APPROVE is the main way to answer in the popup. Herdr passes each
// tap to the popup as text on its keyboard input, a press and then a
// release. The first tap must only select the button, even though it also
// sends a release, and the second tap must approve.
func TestPopupTapApproves(t *testing.T) {
	// Run the popup on a pseudo-terminal at Herdr's size; see
	// TestPopupDiscardsEarlyInput.
	master, terminal := approvaltest.OpenPTY(t)
	approvaltest.SetPopupSize(t, terminal, 37, 14)
	// Find the APPROVE rows the popup will draw at this size.
	_, layout := PopupScreen(View{Rec: approval.Record{ID: "id", ApprovalText: "command"}}, 37, 14)
	result := make(chan string, 1)
	go func() {
		choice, _ := AskPopup(approvaltest.PopupTerminal(t, terminal), terminal, approval.Record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
		result <- choice
	}()
	// Wait for the draw before tapping, or the tap is discarded as early input.
	approvaltest.ReadUntil(t, master, "Typed:")
	// A left-button press and release on the APPROVE row:
	// ESC [<button;column;row, ending in M for press and m for release.
	tap := fmt.Sprintf("\x1b[<0;10;%dM\x1b[<0;10;%dm", layout.ApproveTop, layout.ApproveTop)

	approvaltest.WriteTerminal(t, master, tap)
	approvaltest.ShownBeforeAnswer(t, master, "tap APPROVE again to confirm", result)
	approvaltest.WriteTerminal(t, master, tap)

	if got := approvaltest.Receive(t, result); got != approval.ChoiceYes {
		t.Fatalf("answer %q", got)
	}
}

// The popup spends its time waiting for a tap. A resize must redraw it at
// once, or the buttons stay drawn where taps no longer hit them.
func TestPopupRedrawsWhenResized(t *testing.T) {
	// Run the popup on a pseudo-terminal at Herdr's size; see
	// TestPopupDiscardsEarlyInput. Start it in the background and wait for
	// its first draw.
	master, terminal := approvaltest.OpenPTY(t)
	approvaltest.SetPopupSize(t, terminal, 37, 14)
	result := make(chan string, 1)
	go func() {
		choice, _ := AskPopup(approvaltest.PopupTerminal(t, terminal), terminal, approval.Record{ID: "id", Args: []string{"pr", "create"}}, "1 of 1", 0)
		result <- choice
	}()
	approvaltest.ReadUntil(t, master, "Typed:")

	// Grow the terminal and send the resize signal, as a terminal does. The
	// popup runs in this process, so the signal goes to the test itself.
	approvaltest.SetPopupSize(t, terminal, 60, 20)
	if err := syscall.Kill(os.Getpid(), syscall.SIGWINCH); err != nil {
		t.Fatal(err)
	}

	// At 60 columns the buttons are centered 24 columns in, not 13.
	approvaltest.ShownBeforeAnswer(t, master, strings.Repeat(" ", 24)+"[ APPROVE ]", result)
	approvaltest.WriteTerminal(t, master, "q")
	approvaltest.Receive(t, result)
}
