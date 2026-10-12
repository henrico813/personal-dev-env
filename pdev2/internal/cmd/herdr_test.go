package cmd

import (
	"bufio"
	"encoding/json"
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
