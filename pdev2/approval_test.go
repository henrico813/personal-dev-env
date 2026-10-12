package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"fmt"
	"net"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"syscall"
	"testing"
	"time"
	"unicode/utf8"
	"unsafe"

	"pdev2/internal/approval"
)

// A tap answers only on the rows of the APPROVE and DECLINE buttons, so the
// rows the screen draws and the rows a tap is checked against must agree, and
// every line must fit. In live testing Herdr 0.9.1 gave the popup 37x14 inside
// its 40x16 border, where buttons are two rows tall. A shorter popup gets
// one-row buttons, where a one-row mistake would make a tap on a button miss.
func TestPopupScreenFitsHerdrPane(t *testing.T) {
	rec := approval.Record{ID: "123456789", ApprovalText: strings.Repeat("request text that must be clipped ", 4)}
	for _, size := range []struct{ cols, rows int }{{37, 14}, {40, 16}, {37, 10}} {
		lines, layout := popupScreen(popupView{rec: rec, queue: "1 of 1"}, size.cols, size.rows)

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
			lines, _ := popupScreen(popupView{rec: tc.rec, queue: "1 of 1"}, 37, 14)

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

	lines, _ := popupScreen(popupView{rec: rec, queue: "1 of 1"}, 37, 14)

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

	lines, _ := popupScreen(popupView{rec: rec, queue: "1 of 1"}, 40, 16)

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
	master, terminal := openPTY(t)
	setPopupSize(t, terminal, 37, 14)
	// Type yes before the popup starts; it must be discarded.
	if _, err := master.WriteString("yes\n"); err != nil {
		t.Fatal(err)
	}
	// Start the popup in the background; it blocks until it gets an answer.
	result := make(chan string, 1)
	go func() {
		choice, _ := askPopup(popupTerminal(t, terminal), terminal, approval.Record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
		result <- choice
	}()
	// Wait for the draw, so the answer below arrives after the early input is
	// thrown away.
	readUntil(t, master, "Typed:")

	if _, err := master.WriteString("no\n"); err != nil {
		t.Fatal(err)
	}

	if got := receive(t, result); got != "no" {
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
			master, terminal := openPTY(t)
			setPopupSize(t, terminal, 37, 14)
			result := make(chan string, 1)
			go func() {
				choice, _ := askPopup(popupTerminal(t, terminal), terminal, approval.Record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
				result <- choice
			}()
			// Wait for the draw; a key sent earlier would be discarded as early input.
			readUntil(t, master, "Typed:")

			if _, err := master.WriteString(dismiss); err != nil {
				t.Fatal(err)
			}

			if got := receive(t, result); got != approval.ChoiceDismiss {
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
	master, terminal := openPTY(t)
	setPopupSize(t, terminal, 37, 14)
	result := make(chan string, 1)
	go func() {
		choice, _ := askPopup(popupTerminal(t, terminal), terminal, approval.Record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
		result <- choice
	}()
	// Wait for the draw; a key sent earlier would be discarded as early input.
	readUntil(t, master, "Typed:")

	if _, err := master.WriteString("y\n"); err != nil {
		t.Fatal(err)
	}

	// The hint proves y was refused without an answer; q then ends the popup
	// so the test can check that nothing was chosen.
	readUntil(t, master, "Type yes or no, then Enter")
	if _, err := master.WriteString("q"); err != nil {
		t.Fatal(err)
	}
	if got := receive(t, result); got != approval.ChoiceDismiss {
		t.Fatalf("answer %q", got)
	}
}

// The tap rules decide whether a stray tap answers a request. Accidental phone
// taps decided two requests in live use, so one tap only selects a button and
// a second tap on the same button answers. A tap's release is not a second
// tap. For 600 ms after each request appears, taps and Enter are ignored and
// "one moment..." stays shown, even while keys are typed, so a fast "yes"
// and Enter that does nothing still says why.
func TestPopupAppliesTapAndSettleRules(t *testing.T) {
	_, layout := popupScreen(popupView{rec: approval.Record{ID: "id"}}, 37, 14)
	press := popupInput{b: keyEscape, isMouse: true, mouse: mouseReport{x: 10, y: layout.approveTop, press: true}}
	release := popupInput{b: keyEscape, isMouse: true, mouse: mouseReport{x: 10, y: layout.approveTop}}
	declinePress := popupInput{b: keyEscape, isMouse: true, mouse: mouseReport{x: 10, y: layout.declineTop, press: true}}
	typedYes := []popupInput{{b: 'y'}, {b: 'e'}, {b: 's'}, {b: '\r'}}
	// "yess", Backspace, then Enter, so both editing keys run during the pause.
	correctedYes := []popupInput{{b: 'y'}, {b: 'e'}, {b: 's'}, {b: 's'}, {b: keyBackspace}, {b: '\r'}}
	cases := []struct {
		name     string
		settling bool
		inputs   []popupInput
		want     string // The choice after the last input; earlier inputs must give none.
	}{
		{name: "second tap answers", inputs: []popupInput{press, press}, want: approval.ChoiceYes},
		{name: "switch to decline", inputs: []popupInput{press, declinePress, declinePress}, want: approval.ChoiceNo},
		{name: "release does not confirm", inputs: []popupInput{press, release}, want: approval.ChoiceNone},
		{name: "settling ignores taps", settling: true, inputs: []popupInput{press, press}, want: approval.ChoiceNone},
		{name: "Enter answers typed yes", inputs: typedYes, want: approval.ChoiceYes},
		{name: "settling ignores Enter", settling: true, inputs: correctedYes, want: approval.ChoiceNone},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			// askPopup shows the pause notice when the popup opens.
			v := &popupView{}
			if tc.settling {
				v.notice = settleNotice
			}

			got := approval.ChoiceNone
			for i, input := range tc.inputs {
				if got != approval.ChoiceNone {
					t.Fatalf("answered %q before input %d", got, i)
				}
				got, _ = v.handle(input, layout, tc.settling)
				if tc.settling && v.notice != settleNotice {
					t.Fatalf("notice after input %d = %q, want %q", i, v.notice, settleNotice)
				}
			}

			if got != tc.want {
				t.Fatalf("choice = %q, want %q", got, tc.want)
			}
		})
	}
}

// Fails if the popup answers before want appears on screen.
func shownBeforeAnswer[T any](t *testing.T, master *os.File, want string, answered chan T) {
	t.Helper()
	shown := make(chan bool, 1)
	go func() { shown <- readsUntil(master, want) }()
	select {
	case got := <-answered:
		t.Fatalf("answered %v before showing %q", got, want)
	case ok := <-shown:
		if !ok {
			t.Fatal("popup output ended")
		}
	case <-time.After(5 * time.Second):
		t.Fatalf("%q was never shown", want)
	}
}

// Tapping APPROVE is the main way to answer in the popup. Herdr passes each
// tap to the popup as text on its keyboard input, a press and then a
// release. The first tap must only select the button, even though it also
// sends a release, and the second tap must approve.
func TestPopupTapApproves(t *testing.T) {
	// Run the popup on a pseudo-terminal at Herdr's size; see
	// TestPopupDiscardsEarlyInput.
	master, terminal := openPTY(t)
	setPopupSize(t, terminal, 37, 14)
	// Find the APPROVE rows the popup will draw at this size.
	_, layout := popupScreen(popupView{rec: approval.Record{ID: "id", ApprovalText: "command"}}, 37, 14)
	result := make(chan string, 1)
	go func() {
		choice, _ := askPopup(popupTerminal(t, terminal), terminal, approval.Record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
		result <- choice
	}()
	// Wait for the draw before tapping, or the tap is discarded as early input.
	readUntil(t, master, "Typed:")
	// A left-button press and release on the APPROVE row:
	// ESC [<button;column;row, ending in M for press and m for release.
	tap := fmt.Sprintf("\x1b[<0;10;%dM\x1b[<0;10;%dm", layout.approveTop, layout.approveTop)

	writeTerminal(t, master, tap)
	shownBeforeAnswer(t, master, "tap APPROVE again to confirm", result)
	writeTerminal(t, master, tap)

	if got := receive(t, result); got != approval.ChoiceYes {
		t.Fatalf("answer %q", got)
	}
}

// The popup spends its time waiting for a tap. A resize must redraw it at
// once, or the buttons stay drawn where taps no longer hit them.
func TestPopupRedrawsWhenResized(t *testing.T) {
	// Run the popup on a pseudo-terminal at Herdr's size; see
	// TestPopupDiscardsEarlyInput. Start it in the background and wait for
	// its first draw.
	master, terminal := openPTY(t)
	setPopupSize(t, terminal, 37, 14)
	result := make(chan string, 1)
	go func() {
		choice, _ := askPopup(popupTerminal(t, terminal), terminal, approval.Record{ID: "id", Args: []string{"pr", "create"}}, "1 of 1", 0)
		result <- choice
	}()
	readUntil(t, master, "Typed:")

	// Grow the terminal and send the resize signal, as a terminal does. The
	// popup runs in this process, so the signal goes to the test itself.
	setPopupSize(t, terminal, 60, 20)
	if err := syscall.Kill(os.Getpid(), syscall.SIGWINCH); err != nil {
		t.Fatal(err)
	}

	// At 60 columns the buttons are centered 24 columns in, not 13.
	shownBeforeAnswer(t, master, strings.Repeat(" ", 24)+"[ APPROVE ]", result)
	writeTerminal(t, master, "q")
	receive(t, result)
}

// On a phone, an impatient second tap lands on the next request, which has
// just appeared under the finger, and would approve it unread. Each queued
// request therefore opens with a settle pause and no selected button.
// TestPopupAppliesTapAndSettleRules checks that taps during the pause are
// ignored; this test checks that the queue applies the pause and the reset
// to every request, not only the first.
func TestQueuedPopupsSettleAndStartUnselected(t *testing.T) {
	master, layout := queueTwoMerges(t)
	press := fmt.Sprintf("\x1b[<0;10;%dM", layout.approveTop)
	// Run the queue in the background; approvePopup returns once no
	// undismissed request is left.
	done := make(chan int, 1)
	go func() { done <- approvePopup("first") }()

	// The first popup settles too; wait for the redraw that clears its notice.
	readUntil(t, master, "Typed:")
	shownBeforeAnswer(t, master, "Typed:", done)
	// Approve the first request with a selecting tap and a confirming tap.
	writeTerminal(t, master, press)
	shownBeforeAnswer(t, master, "tap APPROVE again to confirm", done)
	writeTerminal(t, master, press)
	// The second request opens with the pause notice. Wait for the redraw that
	// clears it, so the taps below land after the pause.
	readUntil(t, master, settleNotice)
	shownBeforeAnswer(t, master, "Typed:", done)
	if second, err := approval.Load("second"); err != nil || second.State != approval.StatePending {
		t.Fatalf("second request = %#v, %v; want pending", second, err)
	}
	// The first request's confirmed selection must not carry over, so one tap
	// only selects.
	writeTerminal(t, master, press)
	shownBeforeAnswer(t, master, "tap APPROVE again to confirm", done)
	writeTerminal(t, master, press)

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
func queueTwoMerges(t *testing.T) (*os.File, popupLayout) {
	t.Helper()
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	now := time.Now()
	for i, id := range []string{"first", "second"} {
		rec := approval.Record{ID: id, Args: []string{"pr", "merge", fmt.Sprint(i + 1)}, State: approval.StatePending, Created: now.Add(time.Duration(i) * time.Second)}
		if err := approval.Save(rec); err != nil {
			t.Fatal(err)
		}
	}
	master, terminal := openPTY(t)
	setPopupSize(t, terminal, 37, 14)
	oldIn, oldOut := popupIn, popupOut
	popupIn, popupOut = popupTerminal(t, terminal), terminal
	t.Cleanup(func() { popupIn, popupOut = oldIn, oldOut })
	_, layout := popupScreen(popupView{rec: approval.Record{Args: []string{"pr", "merge", "1"}}, queue: "1 of 2"}, 37, 14)
	return master, layout
}

func writeTerminal(t *testing.T, master *os.File, text string) {
	t.Helper()
	if _, err := master.WriteString(text); err != nil {
		t.Fatal(err)
	}
}

// readUntil without t, for use from a goroutine.
func readsUntil(master *os.File, want string) bool {
	buf := make([]byte, 1)
	var got bytes.Buffer
	for !bytes.Contains(got.Bytes(), []byte(want)) {
		if _, err := master.Read(buf); err != nil {
			return false
		}
		got.Write(buf)
	}
	return true
}

// A phone answer counts only while the request is pending, so a late yes
// cannot touch a request that already ran. Whatever happens, the phone
// question removes its PID when it ends: the system reuses PIDs, and a stale
// one could look like a running question and stop reruns from sending a new
// one. Removing the PID alone must keep the record's age, or a request whose
// phone question keeps failing would never expire.
func TestMoshiAnswersPendingRequest(t *testing.T) {
	for _, tc := range []struct {
		name, hook, state, want string
		ageKept                 bool
	}{
		{"approve", "exit 0", approval.StatePending, approval.StateApproved, false},
		{"decline", "exit 1", approval.StatePending, approval.StateDeclined, false},
		{"handled", "exit 0", approval.StateRan, approval.StateRan, true},
		{"failed ask", "kill -9 $$", approval.StatePending, approval.StatePending, true},
	} {
		t.Run(tc.name, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			// A fake moshi-hook on PATH answers with the case's exit; kill -9
			// stands for an ask that died without an answer.
			dir := t.TempDir()
			fake := filepath.Join(dir, "moshi-hook")
			if err := os.WriteFile(fake, []byte("#!/bin/sh\n"+tc.hook+"\n"), 0o755); err != nil {
				t.Fatal(err)
			}
			t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
			// moshiAsk runs in this test process, so the PID saved for the
			// question is the test's own; the record was last changed an hour ago.
			saved := time.Now().Add(-time.Hour).Round(0)
			rec := approval.Record{ID: tc.name, State: tc.state, Updated: saved, MoshiPID: os.Getpid()}
			if err := approval.Write(rec); err != nil {
				t.Fatal(err)
			}

			code := moshiAsk([]string{rec.ID})

			if code != 0 {
				t.Fatalf("moshiAsk returned %d", code)
			}
			got, err := approval.Load(rec.ID)
			if err != nil {
				t.Fatal(err)
			}
			if got.State != tc.want || got.MoshiPID != 0 {
				t.Fatalf("state %q with PID %d, want %q and no PID", got.State, got.MoshiPID, tc.want)
			}
			if kept := got.Updated.Equal(saved); kept != tc.ageKept {
				t.Errorf("age kept = %t, want %t", kept, tc.ageKept)
			}
		})
	}
}

// A phone question outlived popup answers by up to four hours, leaving a
// question on the phone that could no longer decide anything.
func TestAnsweredRequestStopsPhoneAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake moshi-hook never answers, like a phone nobody looks at. It
	// runs as a child of the ask, so only a signal to the ask's whole group
	// reaches it. It touches ready once it can catch the signal, and stopped
	// when it gets one.
	dir := t.TempDir()
	ready, stopped := filepath.Join(dir, "ready"), filepath.Join(dir, "stopped")
	writeScript(t, dir, "moshi-hook", `trap 'touch "`+stopped+`"; exit 0' TERM; touch "`+ready+`"; sleep 30 & wait`)
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	old := moshiProgram
	moshiProgram = func() (string, error) { return writeScript(t, dir, "ask", `moshi-hook ask "$@"`), nil }
	t.Cleanup(func() { moshiProgram = old })
	// Start a real detached ask for a saved request.
	rec := approval.Record{ID: "popup", State: approval.StatePending}
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
	}
	rec = startMoshi(rec)
	if rec.MoshiPID == 0 {
		t.Fatal("phone ask did not start")
	}
	t.Cleanup(func() { _ = syscall.Kill(-rec.MoshiPID, syscall.SIGKILL) })
	waitForFile(t, ready)

	// Answer as the popup does; that must stop the ask.
	saveAnswer(rec.ID, approval.ChoiceYes)

	waitForFile(t, stopped)
}

// Agents rerun a waiting command to learn what happened. While the first
// phone question is still waiting, a rerun must not send another, or the phone
// would show several questions for one request.
func TestMoshiRerunKeepsLiveAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	fake := filepath.Join(t.TempDir(), "ask")
	if err := os.WriteFile(fake, []byte("#!/bin/sh\nsleep 30\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	// The fake question program just sleeps, standing in for a phone question
	// still waiting for an answer.
	old := moshiProgram
	moshiProgram = func() (string, error) { return fake, nil }
	t.Cleanup(func() { moshiProgram = old })
	discardStderr(t)
	// Create the request, which starts the first question.
	req := approval.Request{ID: "live", Args: []string{"pr", "comment", "12"}}
	if _, err := loadOrCreate(req, ""); err != nil {
		t.Fatal(err)
	}
	first, err := approval.Load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	// The question runs in its own session, outside the test's process group,
	// so stop its whole group at the end.
	if first.MoshiPID != 0 {
		t.Cleanup(func() { _ = syscall.Kill(-first.MoshiPID, syscall.SIGKILL) })
	}

	// Rerun the same request while the first question is still running.
	state, err := loadOrCreate(req, "")

	if err != nil || state != approval.StatePending {
		t.Fatalf("rerun state %q, err %v", state, err)
	}
	second, err := approval.Load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	if first.MoshiPID == 0 || second.MoshiPID != first.MoshiPID || !processAlive(first.MoshiPID) {
		t.Fatalf("PIDs %d and %d", first.MoshiPID, second.MoshiPID)
	}
}

// In live testing a phone question ended before it reached the phone, leaving
// the request waiting with nothing to answer there. A rerun must send a new
// question when the saved one is no longer running. That must keep the
// record's age; otherwise a request whose question keeps failing would stay
// pending forever as the agent reruns it.
func TestMoshiRerunRestartsDeadAsk(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake question program just sleeps, standing in for a new question
	// waiting for an answer.
	dir := t.TempDir()
	fake := filepath.Join(dir, "ask")
	if err := os.WriteFile(fake, []byte("#!/bin/sh\nsleep 30\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	old := moshiProgram
	moshiProgram = func() (string, error) { return fake, nil }
	t.Cleanup(func() { moshiProgram = old })
	// The saved PID belongs to a process that has already exited, and the
	// request was made three hours ago.
	saved := time.Now().Add(-3 * time.Hour).Round(0)
	rec := approval.Record{ID: "dead", MoshiPID: deadPID(t), State: approval.StatePending, Updated: saved}
	if err := approval.Write(rec); err != nil {
		t.Fatal(err)
	}

	state, err := loadOrCreate(approval.Request{ID: rec.ID, Args: []string{"pr", "comment", "12"}}, "")

	if err != nil || state != approval.StatePending {
		t.Fatalf("rerun state %q, err %v", state, err)
	}
	got, err := approval.Load(rec.ID)
	if err != nil {
		t.Fatal(err)
	}
	// Stop the new question's process group at the end; it would otherwise
	// sleep for 30 seconds.
	if got.MoshiPID != 0 {
		t.Cleanup(func() { _ = syscall.Kill(-got.MoshiPID, syscall.SIGKILL) })
	}
	if got.MoshiPID == rec.MoshiPID || !processAlive(got.MoshiPID) {
		t.Fatalf("record %#v", got)
	}
	if !got.Updated.Equal(saved) {
		t.Errorf("rerun moved the last change from %v to %v", saved, got.Updated)
	}
}

// A fixed large PID can belong to a live process. A child that has exited and
// been waited for has a free PID, unless the kernel has already reused it.
func deadPID(t *testing.T) int {
	t.Helper()
	cmd := exec.Command("true")
	if err := cmd.Run(); err != nil {
		t.Fatal(err)
	}
	return cmd.Process.Pid
}

func setPopupSize(t *testing.T, terminal *os.File, cols, rows uint16) {
	t.Helper()
	size := struct{ rows, cols, x, y uint16 }{rows: rows, cols: cols}
	if _, _, errno := syscall.Syscall(syscall.SYS_IOCTL, terminal.Fd(), syscall.TIOCSWINSZ, uintptr(unsafe.Pointer(&size))); errno != 0 {
		t.Fatal(errno)
	}
}

func popupTerminal(t *testing.T, terminal *os.File) *os.File {
	t.Helper()
	fd, err := syscall.Dup(int(terminal.Fd()))
	if err != nil {
		t.Fatal(err)
	}
	popup := os.NewFile(uintptr(fd), "popup-terminal")
	// The popup gets its own copy because each *os.File closes its descriptor;
	// sharing one number could later close a reused one, such as a request lock.
	t.Cleanup(func() { _ = popup.Close() })
	return popup
}

// readUntil reads the popup's screen until want appears, failing after 2
// seconds. Tests wait for the drawn screen before typing, because the popup
// throws away anything typed before it draws.
func readUntil(t *testing.T, master *os.File, want string) {
	t.Helper()
	found := make(chan error, 1)
	go func() {
		buf := make([]byte, 1)
		var got bytes.Buffer
		for !bytes.Contains(got.Bytes(), []byte(want)) {
			if _, err := master.Read(buf); err != nil {
				found <- err
				return
			}
			got.Write(buf)
		}
		found <- nil
	}()
	select {
	case err := <-found:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(2 * time.Second):
		t.Fatalf("terminal never showed %q", want)
	}
}

// receive fails the test if nothing arrives within 2 seconds.
func receive[T any](t *testing.T, values <-chan T) T {
	t.Helper()
	select {
	case value := <-values:
		return value
	case <-time.After(2 * time.Second):
		t.Fatal("timed out after 2s")
	}
	var zero T
	return zero
}

// Inside Herdr, a new request must open the approval popup in front of the
// user. The herdr command has no option that opens a popup, so the program
// sends the request to Herdr's socket. It must ask for a focused popup and pass
// the request ID that the popup needs to find the request.
func TestHerdrPopupUsesSocketRequest(t *testing.T) {
	// A Unix socket in a temporary directory stands in for Herdr's.
	socket := filepath.Join(t.TempDir(), "herdr.sock")
	listener, err := net.Listen("unix", socket)
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	// The fake herdr CLI reports that socket for "herdr status server" and
	// ignores the other calls.
	dir := t.TempDir()
	fake := filepath.Join(dir, "herdr")
	script := "#!/bin/sh\nif [ \"$1\" = status ]; then echo socket: $HERDR_TEST_SOCKET; fi\n"
	if err := os.WriteFile(fake, []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	t.Setenv("HERDR_ENV", "1")
	t.Setenv("HERDR_TEST_SOCKET", socket)
	// notifyRequest opens the popup only for a request still waiting, so save
	// one.
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	rec := approval.Record{ID: "request-id", State: approval.StatePending}
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
	}
	// Accept one request and reply, as Herdr does, so sendHerdr returns.
	received := make(chan map[string]any, 1)
	go func() {
		var message map[string]any
		defer func() { received <- message }()
		conn, err := listener.Accept()
		if err != nil {
			return
		}
		defer conn.Close()
		if json.NewDecoder(bufio.NewReader(conn)).Decode(&message) == nil {
			_, _ = conn.Write([]byte("{}\n"))
		}
	}()

	notifyRequest(rec)

	message := receive(t, received)
	params, ok := message["params"].(map[string]any)
	if !ok {
		t.Fatal("missing params")
	}
	if message["method"] != "plugin.pane.open" || params["placement"] != "popup" || params["focus"] != true {
		t.Fatalf("request = %#v", message)
	}
	env, ok := params["env"].(map[string]any)
	if !ok || env["PDE_REQUEST_ID"] != "request-id" {
		t.Fatalf("env = %#v", params["env"])
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

// One agent pane can ask twice; answering one request must not hide that
// the other is still waiting.
func TestMarkerStaysWhileSamePaneWaits(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake herdr logs each call, so the test can see marker changes.
	dir := t.TempDir()
	log := filepath.Join(dir, "log")
	writeScript(t, dir, "herdr", `echo "$*" >> '`+log+`'`)
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	for _, id := range []string{"first", "second"} {
		if err := approval.Save(approval.Record{ID: id, State: approval.StatePending, PaneID: "w1:p2"}); err != nil {
			t.Fatal(err)
		}
	}

	saveAnswer("first", approval.ChoiceNo)
	afterFirst, _ := os.ReadFile(log)
	saveAnswer("second", approval.ChoiceNo)
	afterSecond, _ := os.ReadFile(log)

	if len(afterFirst) != 0 {
		t.Fatalf("marker changed while a request still waits: %q", afterFirst)
	}
	if !strings.Contains(string(afterSecond), "pane report-metadata --source pde.approval w1:p2 --clear-title") {
		t.Fatalf("marker not cleared after the last answer: %q", afterSecond)
	}
}

// The phone answers from its own process, not through the popup or a
// terminal. Its answer must clear "Approval waiting" too, or the marker stays
// on the agent's pane after the request is decided.
func TestPhoneAnswerClearsMarker(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake moshi-hook approves at once; the fake herdr logs each call.
	dir := t.TempDir()
	log := filepath.Join(dir, "log")
	writeScript(t, dir, "moshi-hook", "exit 0")
	writeScript(t, dir, "herdr", `echo "$*" >> '`+log+`'`)
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	if err := approval.Save(approval.Record{ID: "phone", State: approval.StatePending, PaneID: "w1:p2"}); err != nil {
		t.Fatal(err)
	}

	moshiAsk([]string{"phone"})

	calls, _ := os.ReadFile(log)
	if !strings.Contains(string(calls), "pane report-metadata --source pde.approval w1:p2 --clear-title") {
		t.Fatalf("herdr calls = %q", calls)
	}
}
