package cmd

import (
	"bufio"
	"encoding/json"
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

// Herdr shows one popup at a time. With an approval popup left open in a
// workspace the user was not viewing, Herdr refused every new popup, but
// Ctrl+b a treated the refusal as success: it showed nothing and exited 0,
// and the user believed no approvals were waiting. The inbox must report
// the failure, move to the open popup, and name its workspace. Herdr can
// kill a popup before it removes its saved workspace, so a saved popup
// whose process is gone must not send the user to an empty workspace.
func TestInboxLeadsToOpenApprovalPopup(t *testing.T) {
	tests := []struct {
		name      string
		pid       int
		wantFocus bool
		wantBody  string
	}{
		// This test process stands in for the popup open in workspace w1G.
		{"popup open", os.Getpid(), true, "Approval waiting: answer the popup in hermes"},
		{"popup killed", deadPID(t), false, "Close the open Herdr popup or menu, then press Ctrl+b a"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			if err := approval.Save(approval.Record{ID: "waiting", State: approval.StatePending}); err != nil {
				t.Fatal(err)
			}
			if err := approval.SavePopup(approval.Popup{PID: tt.pid, Workspace: "w1G"}); err != nil {
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

			code := openInbox()

			calls, _ := os.ReadFile(log)
			if code != 1 {
				t.Errorf("exit = %d, want 1", code)
			}
			if got := strings.Contains(string(calls), "workspace focus w1G"); got != tt.wantFocus {
				t.Errorf("moved to w1G = %v, want %v: %q", got, tt.wantFocus, calls)
			}
			if !strings.Contains(string(calls), "notification show PR approval --body "+tt.wantBody) {
				t.Errorf("notification does not say %q: %q", tt.wantBody, calls)
			}
		})
	}
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
