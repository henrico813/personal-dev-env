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

func TestPopupScreenFitsHerdrPane(t *testing.T) {
	// Herdr 0.9.1 gave the live popup 37x14 inside its configured 40x16 border.
	// A shorter pane draws one-row buttons, where a tap must hit exactly.
	rec := record{ID: "123456789", ApprovalText: strings.Repeat("request text that must be clipped ", 4)}
	for _, size := range []struct{ cols, rows int }{{37, 14}, {40, 16}, {37, 10}} {
		lines, layout := popupScreen(rec, "", "", size.cols, size.rows)

		if len(lines) != size.rows {
			t.Fatalf("%dx%d rendered %d rows", size.cols, size.rows, len(lines))
		}
		for _, line := range lines {
			if len(line) > size.cols {
				t.Fatalf("%dx%d line exceeded columns: %q", size.cols, size.rows, line)
			}
		}
		if strings.Count(strings.Join(lines, "\n"), "v: full text; q/Esc: dismiss") != 1 {
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

func TestPopupShowsRequestSummary(t *testing.T) {
	// A live 37x14 popup hid the title and body behind generated request metadata.
	rec := record{
		Args:         []string{"pr", "comment", "12"},
		Title:        "needed title",
		Body:         "first body line\nsecond body line",
		ApprovalText: "id: long\ntime: long\ncommand: long\nbody: long",
	}

	lines, _ := popupScreen(rec, "", "", 37, 14)

	rendered := strings.Join(lines, "\n")
	for _, want := range []string{"gh pr comment 12", "Title: needed title", "first body line"} {
		if !strings.Contains(rendered, want) {
			t.Fatalf("summary missing %q: %q", want, rendered)
		}
	}
}

func TestPopupClipsWithoutSplittingCharacters(t *testing.T) {
	// Cutting a title by bytes split a multi-byte character into invalid text.
	rec := record{ID: "id", Args: []string{"pr", "create"}, Title: strings.Repeat("é", 60)}

	lines, _ := popupScreen(rec, "", "", 37, 14)

	for _, line := range lines {
		if !utf8.ValidString(line) || utf8.RuneCountInString(line) > 37 {
			t.Fatalf("line %q is invalid or wider than 37 characters", line)
		}
	}
	if want := "Title: " + strings.Repeat("é", 30); !slices.Contains(lines, want) {
		t.Fatalf("lines %q do not contain the clipped title", lines)
	}
}

func TestEditChangesStopsBeforeBody(t *testing.T) {
	// Markdown bullets in the requested body looked like diff removals in a live edit popup.
	text := editHeader + "Title:\n-old\n+new\n\nbody:\n- bullet\n"

	got := strings.Join(editChanges(text), "\n")

	if got != "-old\n+new" {
		t.Fatalf("changes %q", got)
	}
}

func TestPopupEscapesControlText(t *testing.T) {
	// Agent-supplied control bytes must remain visible instead of changing the screen.
	rec := record{ID: "id", Body: "title\x1b[2J\rbody\u009bend"}

	lines, _ := popupScreen(rec, "", "", 40, 16)

	rendered := strings.Join(lines, "\n")
	if strings.ContainsAny(rendered, "\x1b\r\u009b") || !strings.Contains(rendered, "^[") || !strings.Contains(rendered, "^M") || !strings.Contains(rendered, "\\u009b") {
		t.Fatalf("rendered = %q", rendered)
	}
}

func TestPopupDiscardsEarlyInput(t *testing.T) {
	// Buffered approval text must not decide the popup before it appears.
	master, terminal := openPTY(t)
	setPopupSize(t, terminal, 37, 14)
	// Type yes before the popup starts; it must be discarded.
	if _, err := master.WriteString("yes\n"); err != nil {
		t.Fatal(err)
	}
	result := make(chan string, 1)
	go func() {
		choice, _ := askPopup(popupTerminal(t, terminal), terminal, record{ID: "id", ApprovalText: "command"})
		result <- choice
	}()
	// Wait for the draw, so the answer below arrives after the flush.
	readUntil(t, master, "Typed:")

	if _, err := master.WriteString("no\n"); err != nil {
		t.Fatal(err)
	}

	if got := receive(t, result); got != "no" {
		t.Fatalf("answer %q", got)
	}
}

func TestPopupDismissesWithoutAnswer(t *testing.T) {
	// The live popup trapped users after q and Esc were ignored.
	for _, dismiss := range []string{"q", "\x1b"} {
		t.Run(dismiss, func(t *testing.T) {
			master, terminal := openPTY(t)
			setPopupSize(t, terminal, 37, 14)
			result := make(chan string, 1)
			go func() {
				choice, _ := askPopup(popupTerminal(t, terminal), terminal, record{ID: "id", ApprovalText: "command"})
				result <- choice
			}()
			// Wait for the draw; a key sent earlier would be discarded as early input.
			readUntil(t, master, "Typed:")

			if _, err := master.WriteString(dismiss); err != nil {
				t.Fatal(err)
			}

			if got := receive(t, result); got != "" {
				t.Fatalf("answer %q", got)
			}
		})
	}
}

func TestPopupRejectsLoneY(t *testing.T) {
	// A live user repeatedly entered y without feedback, leaving the popup unclear.
	master, terminal := openPTY(t)
	setPopupSize(t, terminal, 37, 14)
	result := make(chan string, 1)
	go func() {
		choice, _ := askPopup(popupTerminal(t, terminal), terminal, record{ID: "id", ApprovalText: "command"})
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
	if got := receive(t, result); got != "" {
		t.Fatalf("answer %q", got)
	}
}

func TestPopupTapApproves(t *testing.T) {
	// Herdr sends taps as xterm mouse reports (ESC [ < button;x;y M) on the
	// popup's inherited standard input, a blocking file rather than one Go can
	// poll.
	master, terminal := openPTY(t)
	setPopupSize(t, terminal, 37, 14)
	_, layout := popupScreen(record{ID: "id", ApprovalText: "command"}, "", "", 37, 14)
	result := make(chan struct {
		choice string
		err    error
	}, 1)
	go func() {
		choice, err := askPopup(popupTerminal(t, terminal), terminal, record{ID: "id", ApprovalText: "command"})
		result <- struct {
			choice string
			err    error
		}{choice, err}
	}()
	// Wait for the draw before tapping, or the tap is discarded as early input.
	readUntil(t, master, "Typed:")

	// A press and release on the APPROVE row, as one tap.
	if _, err := fmt.Fprintf(master, "\x1b[<0;10;%dM\x1b[<0;10;%dm", layout.approveTop, layout.approveTop); err != nil {
		t.Fatal(err)
	}

	if got := receive(t, result); got.err != nil || got.choice != "yes" {
		t.Fatalf("answer %q, err %v", got.choice, got.err)
	}
}

func TestMoshiAnswersPendingRequest(t *testing.T) {
	// Phone answers approve or decline only while the request is pending. Every
	// ask clears its own PID: a reused PID would look alive and stop reruns from
	// restarting the ask. Clearing it alone must keep the record's age, or a
	// request whose ask keeps failing would never expire.
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
			// moshiAsk runs in this process, so the ask's PID is the test's.
			saved := time.Now().Add(-time.Hour).Round(0)
			rec := record{ID: tc.name, State: tc.state, Updated: saved, MoshiPID: os.Getpid()}
			if err := writeRecord(rec); err != nil {
				t.Fatal(err)
			}

			code := moshiAsk([]string{rec.ID, "question"})

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

func TestMoshiRerunKeepsLiveAsk(t *testing.T) {
	// Retries must reuse one detached ask while that process is alive.
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	fake := filepath.Join(t.TempDir(), "ask")
	if err := os.WriteFile(fake, []byte("#!/bin/sh\nsleep 30\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	// The fake ask just sleeps, standing in for a phone question still waiting.
	old := moshiProgram
	moshiProgram = func() (string, error) { return fake, nil }
	t.Cleanup(func() { moshiProgram = old })
	// Start the first ask, then rerun the request while it is alive.
	req := request{ID: "live", Args: []string{"pr", "comment", "12"}}
	if _, err := loadOrCreate(req, ""); err != nil {
		t.Fatal(err)
	}
	first, err := load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	// The ask has its own session, so kill its whole group at the end.
	if first.MoshiPID != 0 {
		t.Cleanup(func() { _ = syscall.Kill(-first.MoshiPID, syscall.SIGKILL) })
	}

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

func TestMoshiRerunRestartsDeadAsk(t *testing.T) {
	// During live testing, an ask exited before reaching the moshi-hook daemon,
	// leaving the pending request with no question on the phone. Restarting the
	// ask must keep the record's age; otherwise an agent rerunning a request
	// whose ask keeps dying would keep it pending forever.
	t.Setenv("XDG_STATE_HOME", t.TempDir())
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
	// Kill the new ask's group at the end; it would otherwise sleep for 30 seconds.
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
	// Each *os.File closes its descriptor; a shared number could later close a
	// reused one, such as a request lock.
	t.Cleanup(func() { _ = popup.Close() })
	return popup
}

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

func TestHerdrPopupUsesSocketRequest(t *testing.T) {
	// The CLI cannot request popup placement, so the socket request must carry it.
	// A Unix socket in a temp dir stands in for the Herdr server.
	socket := filepath.Join(t.TempDir(), "herdr.sock")
	listener, err := net.Listen("unix", socket)
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	// The fake herdr CLI reports that socket for "herdr status server" and
	// ignores the notification call.
	dir := t.TempDir()
	fake := filepath.Join(dir, "herdr")
	script := "#!/bin/sh\nif [ \"$1\" = status ]; then echo socket: $HERDR_TEST_SOCKET; fi\n"
	if err := os.WriteFile(fake, []byte(script), 0o755); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	t.Setenv("HERDR_ENV", "1")
	t.Setenv("HERDR_TEST_SOCKET", socket)
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

	notifyRequest(record{ID: "request-id"})

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
