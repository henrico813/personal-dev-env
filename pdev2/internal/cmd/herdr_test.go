package cmd

import (
	"bufio"
	"encoding/json"
	"fmt"
	"maps"
	"net"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
)

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
	approvaltest.FakeHerdr(t, `if [ "$1" = status ]; then echo socket: $HERDR_TEST_SOCKET; fi`)
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

	message := approvaltest.Receive(t, received)
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

// An agent on another computer asked for approval. The owner's Herdr,
// attached to that computer, showed only "Approval is waiting", and the
// popup was drawn there, out of sight, so merges stayed blocked. The
// notification must name the computer, and the workspace the popup opened
// in: Herdr's focused one, not the agent's.
func TestNotificationNamesComputerAndPopupWorkspace(t *testing.T) {
	host, err := os.Hostname()
	if err != nil {
		t.Fatal(err)
	}
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	rec := approval.Record{ID: "request-id", State: approval.StatePending, Workspace: "agent-work"}
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
	}
	// Herdr's socket opens the popup.
	socket := filepath.Join(t.TempDir(), "herdr.sock")
	listener, err := net.Listen("unix", socket)
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	go func() {
		conn, err := listener.Accept()
		if err != nil {
			return
		}
		defer conn.Close()
		if _, err := bufio.NewReader(conn).ReadBytes('\n'); err == nil {
			_, _ = conn.Write([]byte("{}\n"))
		}
	}()
	// The fake herdr reports the socket and lists the agent's workspace
	// before the focused one, and logs every call.
	log := approvaltest.FakeHerdr(t, `echo "$*" >> "$log"
case "$1 $2" in
"status server") echo socket: $HERDR_TEST_SOCKET ;;
"workspace list") echo '{"result":{"workspaces":[{"workspace_id":"w1","label":"agent-work","focused":false},{"workspace_id":"w3","label":"review","focused":true}]}}' ;;
esac`)
	t.Setenv("HERDR_ENV", "1")
	t.Setenv("HERDR_TEST_SOCKET", socket)

	notifyRequest(rec)

	calls, _ := os.ReadFile(log)
	want := "notification show PR approval on " + host + " --body Answer the popup in review"
	if !strings.Contains(string(calls), want) {
		t.Fatalf("notification is not %q: %q", want, calls)
	}
}

// Herdr shows one popup at a time, so a popup open in another workspace
// blocks every approval. The inbox must say where that popup is and go
// there, or the user cannot find it.
func TestInboxMovesToOpenPopup(t *testing.T) {
	tests := []struct {
		name     string
		popup    string
		wantBody string
	}{
		{"approval popup", `{"pid":%d,"workspace_id":"w1G"}`, "Approval waiting: answer the popup in hermes"},
		{"other tool's popup", `{"pid":%d,"workspace_id":"w1G","name":"Comment review"}`, "Comment review is open in hermes. Close it, then press Ctrl+b a"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// This test process stands in for the popup open in workspace w1G.
			code, calls := openInboxWhileHerdrRefuses(t, fmt.Sprintf(tt.popup, os.Getpid()))

			if code != 1 {
				t.Errorf("exit = %d, want 1", code)
			}
			if !strings.Contains(calls, "workspace focus w1G") {
				t.Errorf("did not move to w1G: %q", calls)
			}
			if !strings.Contains(calls, "notification show PR approval --body "+tt.wantBody) {
				t.Errorf("notification does not say %q: %q", tt.wantBody, calls)
			}
		})
	}
}

// Herdr can kill a popup before it removes its file. The inbox must ignore
// that file, or it sends the user to an empty workspace.
func TestKilledPopupFileIsIgnored(t *testing.T) {
	popup := fmt.Sprintf(`{"pid":%d,"workspace_id":"w1G","name":"Comment review"}`, deadPID(t))

	code, calls := openInboxWhileHerdrRefuses(t, popup)

	wantBody := "Close the open Herdr popup or menu, then press Ctrl+b a"
	if code != 1 {
		t.Errorf("exit = %d, want 1", code)
	}
	if strings.Contains(calls, "workspace focus w1G") {
		t.Errorf("moved to the killed popup's workspace: %q", calls)
	}
	if !strings.Contains(calls, "notification show PR approval --body "+wantBody) {
		t.Errorf("notification does not say %q: %q", wantBody, calls)
	}
}

// Runs the inbox with one request waiting while Herdr refuses its popup as
// busy, and returns the exit code and every herdr call. Herdr labels
// workspace w1G "hermes".
func openInboxWhileHerdrRefuses(t *testing.T, popup string) (code int, calls string) {
	t.Helper()
	state := t.TempDir()
	t.Setenv("XDG_STATE_HOME", state)
	discardStderr(t)
	if err := approval.Save(approval.Record{ID: "waiting", State: approval.StatePending}); err != nil {
		t.Fatal(err)
	}
	// Other tools write this JSON, so a renamed key or moved file must fail.
	popupFile := filepath.Join(state, "pde", "pr-write", "open-popup")
	if err := os.WriteFile(popupFile, []byte(popup), 0o600); err != nil {
		t.Fatal(err)
	}
	// Herdr's socket refuses the popup as it does while another one is open.
	socket := filepath.Join(t.TempDir(), "herdr.sock")
	listener, err := net.Listen("unix", socket)
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	go func() {
		conn, err := listener.Accept()
		if err != nil {
			return
		}
		defer conn.Close()
		if _, err := bufio.NewReader(conn).ReadBytes('\n'); err == nil {
			_, _ = conn.Write([]byte(`{"id":"waiting","error":{"code":"ui_busy","message":"a popup pane is already open"}}` + "\n"))
		}
	}()
	// The fake herdr CLI reports the socket and the workspace's label, and
	// logs every call.
	log := approvaltest.FakeHerdr(t, `echo "$*" >> "$log"
case "$1 $2" in
"status server") echo socket: $HERDR_TEST_SOCKET ;;
"workspace list") echo '{"result":{"workspaces":[{"workspace_id":"w1G","label":"hermes"}]}}' ;;
esac`)
	t.Setenv("HERDR_TEST_SOCKET", socket)

	code = openInbox()

	output, _ := os.ReadFile(log)
	return code, string(output)
}

// Ctrl+b a once said "No approvals waiting" while a request waited on
// another machine, and merges stayed blocked. With a saved machine, the
// inbox must still open a request waiting here first, and only with none
// here ask the popup to check the other machines.
func TestInboxPopupChecksOtherMachines(t *testing.T) {
	tests := []struct {
		name    string
		waiting bool
		wantEnv map[string]any
	}{
		{"request here", true, map[string]any{"PDE_REQUEST_ID": "waiting"}},
		{"none here", false, map[string]any{"PDE_CHECK_MACHINES": "1"}},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			if tt.waiting {
				if err := approval.Save(approval.Record{ID: "waiting", State: approval.StatePending}); err != nil {
					t.Fatal(err)
				}
			}
			// A Unix socket stands in for Herdr's and accepts the popup.
			socket := filepath.Join(t.TempDir(), "herdr.sock")
			listener, err := net.Listen("unix", socket)
			if err != nil {
				t.Fatal(err)
			}
			defer listener.Close()
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
			// The fake herdr reports the socket and one saved, enabled machine.
			approvaltest.FakeHerdr(t, `case "$*" in
"status server") echo socket: $HERDR_TEST_SOCKET ;;
"machine list --json") echo '[{"id":"id-wallace","label":"Wallace","enabled":true}]' ;;
esac`)
			t.Setenv("HERDR_TEST_SOCKET", socket)

			code := openInbox()

			message := approvaltest.Receive(t, received)
			params, _ := message["params"].(map[string]any)
			env, _ := params["env"].(map[string]any)
			if code != 0 {
				t.Errorf("exit = %d, want 0", code)
			}
			if !maps.Equal(env, tt.wantEnv) {
				t.Errorf("popup env = %#v, want %#v", env, tt.wantEnv)
			}
		})
	}
}

// Herdr runs Ctrl+b a with its server's PATH. On a computer whose Herdr
// started without ~/.local/bin on PATH, the inbox could not find herdr and
// showed nothing, so a dismissed request could not be reopened. The inbox
// must use the herdr installed beside it.
func TestInboxFindsHerdrBesideItself(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	if err := approval.Save(approval.Record{ID: "dismissed", State: approval.StatePending, Dismissed: true}); err != nil {
		t.Fatal(err)
	}
	// A Unix socket stands in for Herdr's and accepts the popup.
	socket := filepath.Join(t.TempDir(), "herdr.sock")
	listener, err := net.Listen("unix", socket)
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
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
	// herdr is beside the program, as both are in ~/.local/bin, and PATH
	// has no herdr.
	bin := t.TempDir()
	approvaltest.WriteScript(t, bin, "herdr", `if [ "$1" = status ]; then echo socket: $HERDR_TEST_SOCKET; fi`)
	approvaltest.Replace(t, &thisProgram, func() (string, error) { return filepath.Join(bin, "pde-pr-approve"), nil })
	t.Setenv("PATH", t.TempDir())
	t.Setenv("HERDR_TEST_SOCKET", socket)

	code := openInbox()

	if code != 0 {
		t.Fatalf("exit = %d, want 0", code)
	}
	if message := approvaltest.Receive(t, received); message["method"] != "plugin.pane.open" {
		t.Fatalf("request = %#v", message)
	}
}

// One agent pane can ask twice; answering one request must not hide that
// the other is still waiting.
func TestMarkerStaysWhileSamePaneWaits(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake herdr logs each call, so the test can see marker changes.
	log := approvaltest.FakeHerdr(t, `echo "$*" >> "$log"`)
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
