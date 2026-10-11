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
)

// A tap answers only on the rows of the APPROVE and DECLINE buttons, so the
// rows the screen draws and the rows a tap is checked against must agree, and
// every line must fit. In live testing Herdr 0.9.1 gave the popup 37x14 inside
// its 40x16 border. A shorter popup gets one-row buttons, where a one-row
// mistake would make a tap on a button miss.
func TestPopupScreenFitsHerdrPane(t *testing.T) {
	rec := record{ID: "123456789", ApprovalText: strings.Repeat("request text that must be clipped ", 4)}
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

// The popup has room for only a few lines of the request. In live testing the
// 37x14 popup showed the ID, time, and command lines that start the full
// approval text and hid the title and body the approver needs to decide.
func TestPopupShowsRequestSummary(t *testing.T) {
	rec := record{
		Args:         []string{"pr", "comment", "12"},
		Title:        "needed title",
		Body:         "first body line\nsecond body line",
		ApprovalText: "id: long\ntime: long\ncommand: long\nbody: long",
	}

	lines, _ := popupScreen(popupView{rec: rec, queue: "1 of 1"}, 37, 14)
	rendered := strings.Join(lines, "\n")
	for _, want := range []string{"comment #12", "needed title", "first body line"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("summary missing %q: %q", want, rendered)
		}
	}
}

// Titles can contain characters such as é that take more than one byte.
// Cutting a long title by bytes split such a character and drew invalid text,
// so lines are cut by character.
func TestPopupClipsWithoutSplittingCharacters(t *testing.T) {
	rec := record{ID: "id", Args: []string{"pr", "create"}, Title: strings.Repeat("é", 60)}

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

// For pr edit the popup shows changed lines as "-old" and "+new". In a live
// edit popup, Markdown bullets ("- item") in the new body were shown too and
// looked like removed lines.
func TestEditChangesStopsBeforeBody(t *testing.T) {
	text := editHeader + "Title:\n-old\n+new\n\nbody:\n- bullet\n"

	got := strings.Join(editChanges(text), "\n")

	if got != "-old\n+new" {
		t.Fatalf("changes %q", got)
	}
}

// The agent writes the title and body. If the terminal obeyed control
// characters in them, such as ESC [2J (clear the screen), an agent could wipe
// the popup or draw a different request over it. They must appear as text.
func TestPopupEscapesControlText(t *testing.T) {
	rec := record{ID: "id", Body: "title\x1b[2J\rbody\u009bend"}

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
		choice, _ := askPopup(popupTerminal(t, terminal), terminal, record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
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
				choice, _ := askPopup(popupTerminal(t, terminal), terminal, record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
				result <- choice
			}()
			// Wait for the draw; a key sent earlier would be discarded as early input.
			readUntil(t, master, "Typed:")

			if _, err := master.WriteString(dismiss); err != nil {
				t.Fatal(err)
			}

			if got := receive(t, result); got != choiceDismiss {
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
		choice, _ := askPopup(popupTerminal(t, terminal), terminal, record{ID: "id", ApprovalText: "command"}, "1 of 1", 0)
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
	if got := receive(t, result); got != choiceDismiss {
		t.Fatalf("answer %q", got)
	}
}

// The tap rules decide whether a stray tap answers a request. Accidental phone
// taps decided two requests in live use, so one tap only selects a button and
// a second tap on the same button answers. A tap's release is not a second
// tap, and the settle pause after a request appears ignores taps and Enter.
func TestPopupAppliesTapAndSettleRules(t *testing.T) {
	_, layout := popupScreen(popupView{rec: record{ID: "id"}}, 37, 14)
	press := popupInput{b: keyEscape, isMouse: true, mouse: mouseReport{x: 10, y: layout.approveTop, press: true}}
	release := popupInput{b: keyEscape, isMouse: true, mouse: mouseReport{x: 10, y: layout.approveTop}}
	declinePress := popupInput{b: keyEscape, isMouse: true, mouse: mouseReport{x: 10, y: layout.declineTop, press: true}}
	typedYes := []popupInput{{b: 'y'}, {b: 'e'}, {b: 's'}, {b: '\r'}}
	cases := []struct {
		name     string
		settling bool
		inputs   []popupInput
		want     string // The choice after the last input; earlier inputs must give none.
	}{
		{name: "second tap answers", inputs: []popupInput{press, press}, want: choiceYes},
		{name: "switch to decline", inputs: []popupInput{press, declinePress, declinePress}, want: choiceNo},
		{name: "release does not confirm", inputs: []popupInput{press, release}, want: choiceNone},
		{name: "settling ignores taps", settling: true, inputs: []popupInput{press, press}, want: choiceNone},
		{name: "Enter answers typed yes", inputs: typedYes, want: choiceYes},
		{name: "settling ignores Enter", settling: true, inputs: typedYes, want: choiceNone},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			v := &popupView{}

			got := choiceNone
			for i, input := range tc.inputs {
				if got != choiceNone {
					t.Fatalf("answered %q before input %d", got, i)
				}
				got, _ = v.handle(input, layout, tc.settling)
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
		choice, _ := askPopup(popupTerminal(t, terminal), terminal, record{ID: "id", Args: []string{"pr", "create"}}, "1 of 1", 0)
		result <- choice
	}()
	readUntil(t, master, "Typed:")

	// Grow the terminal and send the resize signal, as a terminal does. The
	// popup runs in this process, so the signal goes to the test itself.
	setPopupSize(t, terminal, 60, 20)
	if err := syscall.Kill(os.Getpid(), syscall.SIGWINCH); err != nil {
		t.Fatal(err)
	}

	// At 60 columns the queue position moves to the right edge of the header.
	shownBeforeAnswer(t, master, fmt.Sprintf("%-53s 1 of 1", "create PR"), result)
	writeTerminal(t, master, "q")
	receive(t, result)
}

// In live testing one APPROVE tap approved and ran two queued requests: the
// press answered the first, the popup showed the second, and the tap's
// release landed on the same APPROVE row.
func TestTapAnswersOnlyOneQueuedRequest(t *testing.T) {
	// No settle pause here, so only the press and release rule is tested.
	master, layout := queueTwoMerges(t, 0)
	press := fmt.Sprintf("\x1b[<0;10;%dM", layout.approveTop)
	// Run the queue in the background; approvePopup returns once no
	// undismissed request is left.
	done := make(chan int, 1)
	go func() { done <- approvePopup("first") }()

	// Wait for the draw before tapping, or the tap is discarded as early input.
	readUntil(t, master, "Typed:")
	writeTerminal(t, master, press)
	shownBeforeAnswer(t, master, "tap APPROVE again to confirm", done)
	writeTerminal(t, master, press)
	// Send the confirming tap's release only once the second request is
	// drawn, as happened live, then dismiss it.
	readUntil(t, master, "merge #2")
	writeTerminal(t, master, fmt.Sprintf("\x1b[<0;10;%dm", layout.approveTop))
	writeTerminal(t, master, "q")
	receive(t, done)

	first, err := load("first")
	if err != nil {
		t.Fatal(err)
	}
	second, err := load("second")
	if err != nil {
		t.Fatal(err)
	}
	if first.State != stateApproved || second.State != statePending {
		t.Fatalf("states = %q and %q, want only the first approved", first.State, second.State)
	}
}

// On a phone, an impatient second tap lands on the next request, which has
// just appeared under the finger, and would approve it unread. Each queued
// request therefore opens with a settle pause and no selected button.
// TestPopupAppliesTapAndSettleRules checks that taps during the pause are
// ignored; this test checks that the queue applies the pause and the reset
// to every request, not only the first.
func TestQueuedPopupsSettleAndStartUnselected(t *testing.T) {
	master, layout := queueTwoMerges(t, 500*time.Millisecond)
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
	if second, err := load("second"); err != nil || second.State != statePending {
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
	if second, err := load("second"); err != nil || second.State != stateApproved {
		t.Fatalf("second request = %#v, %v; want approved", second, err)
	}
}

// queueTwoMerges saves two pending merges, oldest first, and points the popup
// at a new pseudo-terminal with the given settle pause. It returns the
// terminal's master side and the first request's button layout.
func queueTwoMerges(t *testing.T, settle time.Duration) (*os.File, popupLayout) {
	t.Helper()
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	now := time.Now()
	for i, id := range []string{"first", "second"} {
		rec := record{ID: id, Args: []string{"pr", "merge", fmt.Sprint(i + 1)}, State: statePending, Created: now.Add(time.Duration(i) * time.Second)}
		if err := save(rec); err != nil {
			t.Fatal(err)
		}
	}
	master, terminal := openPTY(t)
	setPopupSize(t, terminal, 37, 14)
	oldIn, oldOut, oldSettle := popupIn, popupOut, popupSettle
	popupIn, popupOut, popupSettle = popupTerminal(t, terminal), terminal, settle
	t.Cleanup(func() { popupIn, popupOut, popupSettle = oldIn, oldOut, oldSettle })
	_, layout := popupScreen(popupView{rec: record{Args: []string{"pr", "merge", "1"}}, queue: "1 of 2"}, 37, 14)
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
		{"approve", "exit 0", statePending, stateApproved, false},
		{"decline", "exit 1", statePending, stateDeclined, false},
		{"handled", "exit 0", stateRan, stateRan, true},
		{"failed ask", "kill -9 $$", statePending, statePending, true},
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
			rec := record{ID: tc.name, State: tc.state, Updated: saved, MoshiPID: os.Getpid()}
			if err := writeRecord(rec); err != nil {
				t.Fatal(err)
			}

			code := moshiAsk([]string{rec.ID})

			if code != 0 {
				t.Fatalf("moshiAsk returned %d", code)
			}
			got, err := load(rec.ID)
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
// question on the phone that could no longer decide anything. A phone
// question that records its own answer must not stop itself first.
func TestAnsweredRequestStopsPhoneAsk(t *testing.T) {
	t.Run("popup answer", func(t *testing.T) {
		t.Setenv("XDG_STATE_HOME", t.TempDir())
		// The fake moshi-hook never answers, like a phone nobody looks at.
		dir := t.TempDir()
		writeScript(t, dir, "moshi-hook", "exec sleep 30")
		t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
		old := moshiProgram
		moshiProgram = func() (string, error) { return writeScript(t, dir, "ask", `moshi-hook ask "$@"`), nil }
		t.Cleanup(func() { moshiProgram = old })
		// Start a real detached ask for a saved request, and watch for the
		// ask's exit before answering.
		rec := record{ID: "popup", State: statePending}
		if err := save(rec); err != nil {
			t.Fatal(err)
		}
		rec = startMoshi(rec)
		if rec.MoshiPID == 0 {
			t.Fatal("phone ask did not start")
		}
		t.Cleanup(func() { _ = syscall.Kill(-rec.MoshiPID, syscall.SIGKILL) })
		exited := waitForExit(t, rec.MoshiPID)

		// Answer as the popup does; that must stop the ask.
		saveAnswer(rec.ID, "yes")

		select {
		case <-exited:
		case <-time.After(5 * time.Second):
			t.Fatal("phone ask still running after the popup answered")
		}
	})
	t.Run("phone answer", func(t *testing.T) {
		t.Setenv("XDG_STATE_HOME", t.TempDir())
		dir := t.TempDir()
		// The fake moshi-hook approves at once.
		writeScript(t, dir, "moshi-hook", "exit 0")
		t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
		// Stands in for the ask process that is recording the answer; moshiAsk
		// runs in the test, so it would otherwise find no PID of its own.
		self := exec.Command("sleep", "30")
		self.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
		if err := self.Start(); err != nil {
			t.Fatal(err)
		}
		t.Cleanup(func() { _ = syscall.Kill(-self.Process.Pid, syscall.SIGKILL); _ = self.Wait() })
		if err := save(record{ID: "phone", State: statePending, MoshiPID: self.Process.Pid}); err != nil {
			t.Fatal(err)
		}

		code := moshiAsk([]string{"phone"})

		got, err := load("phone")
		if code != 0 || err != nil || got.State != stateApproved {
			t.Fatalf("moshiAsk returned %d, record %#v, %v", code, got, err)
		}
		// A signal takes effect shortly after it is sent, so watch briefly for
		// the stand-in to become a zombie, which would mean it was stopped.
		for deadline := time.Now().Add(200 * time.Millisecond); time.Now().Before(deadline); time.Sleep(10 * time.Millisecond) {
			if procState(self.Process.Pid) == 'Z' {
				t.Fatal("the phone ask stopped itself while recording its answer")
			}
		}
	})
}

func waitForExit(t *testing.T, pid int) <-chan struct{} {
	t.Helper()
	process, err := os.FindProcess(pid)
	if err != nil {
		t.Fatal(err)
	}
	exited := make(chan struct{})
	go func() { _, _ = process.Wait(); close(exited) }()
	return exited
}

// Returns the state letter from /proc, such as 'S' for sleeping or 'Z' for exited.
func procState(pid int) byte {
	data, err := os.ReadFile(fmt.Sprintf("/proc/%d/stat", pid))
	if err != nil {
		return 0
	}
	_, after, _ := strings.Cut(string(data), ") ")
	if after == "" {
		return 0
	}
	return after[0]
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
	req := request{ID: "live", Args: []string{"pr", "comment", "12"}}
	if _, err := loadOrCreate(req, ""); err != nil {
		t.Fatal(err)
	}
	first, err := load(req.ID)
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

	if err != nil || state != statePending {
		t.Fatalf("rerun state %q, err %v", state, err)
	}
	second, err := load(req.ID)
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
	rec := record{ID: "dead", MoshiPID: deadPID(t), State: statePending, Updated: saved}
	if err := writeRecord(rec); err != nil {
		t.Fatal(err)
	}

	state, err := loadOrCreate(request{ID: rec.ID, Args: []string{"pr", "comment", "12"}}, "")

	if err != nil || state != statePending {
		t.Fatalf("rerun state %q, err %v", state, err)
	}
	got, err := load(rec.ID)
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
	rec := record{ID: "request-id", State: statePending}
	if err := save(rec); err != nil {
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
// the inbox can still find it.
func TestDismissedRequestStaysHidden(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	rec := record{ID: "dismissed", State: statePending}
	if err := save(rec); err != nil {
		t.Fatal(err)
	}

	dismissRequest(rec.ID)

	if got := queuedRecords(); len(got) != 0 {
		t.Fatalf("visible pending = %d", len(got))
	}
	if got := inboxRecords(); len(got) != 1 || got[0].ID != rec.ID {
		t.Fatalf("inbox pending = %#v", got)
	}
}

func TestQueueKeepsCreationOrder(t *testing.T) {
	// Restarting a dead runner or phone ask saves the record; that must not
	// send an older request behind newer ones.
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake gh answers the repository lookup.
	gh := writeScript(t, t.TempDir(), "gh", "echo owner/repo")
	discardStderr(t)
	var ids []string
	for _, args := range [][]string{{"pr", "create", "--title", "older"}, {"pr", "create", "--title", "newer"}} {
		req, err := parse(args, nil)
		if err != nil {
			t.Fatal(err)
		}
		req = scopeRequest(req)
		if _, err := loadOrCreate(req, gh); err != nil {
			t.Fatal(err)
		}
		ids = append(ids, req.ID)
	}

	// Save the older record again, as restarting its runner or ask would.
	older, err := load(ids[0])
	if err != nil {
		t.Fatal(err)
	}
	if err := save(older); err != nil {
		t.Fatal(err)
	}

	queue := queuedRecords()
	if len(queue) != 2 || queue[0].ID != ids[0] {
		t.Fatalf("queue = %#v", queue)
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
		if err := save(record{ID: id, State: statePending, PaneID: "w1:p2"}); err != nil {
			t.Fatal(err)
		}
	}

	saveAnswer("first", choiceNo)
	afterFirst, _ := os.ReadFile(log)
	saveAnswer("second", choiceNo)
	afterSecond, _ := os.ReadFile(log)

	if len(afterFirst) != 0 {
		t.Fatalf("marker changed while a request still waits: %q", afterFirst)
	}
	if !strings.Contains(string(afterSecond), "pane report-metadata --source pde.approval w1:p2 --clear-title") {
		t.Fatalf("marker not cleared after the last answer: %q", afterSecond)
	}
}
