package cmd

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"pdev/internal/approval"
	"pdev/internal/approvaltest"
	"pdev/internal/terminal"
)

// With several sessions asking at once, the first lines are how a person
// tells requests apart. The phone used to ask "Approve gh pr merge in
// current repository (185)?" while the popup showed the command, so the two
// named a request differently and neither said which agent asked. Both now
// start with the same context lines, at the top of the popup.
func TestPopupAndPhoneShowSameContext(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	rec := approval.Record{ID: "ctx", State: approval.StatePending, Args: []string{"pr", "merge", "185"}, Repo: "owner/repo",
		Workspace: "pdev-release", SessionTitle: "OC | release", Title: "Ship it"}
	if err := approval.Write(rec); err != nil {
		t.Fatal(err)
	}
	// The fake moshi-hook saves the question it was asked, which comes after
	// "ask --require-remote --source pde-gh-write --timeout 4h", and declines.
	asked := approvaltest.FakeMoshiHook(t, `printf '%s' "$7" > "$log"; exit 1`)

	lines, _ := terminal.PopupScreen(terminal.View{Rec: rec, Queue: "1 of 2"}, 37, 14)
	moshiAsk([]string{rec.ID})

	sent, err := os.ReadFile(asked)
	if err != nil {
		t.Fatal(err)
	}
	question := string(sent)

	want := []string{"merge #185", "owner/repo", "pdev-release / OC | release", "Ship it"}
	if !strings.Contains(lines[0], "1 of 2") {
		t.Fatalf("header %q lacks queue position", lines[0])
	}
	for i, text := range want {
		if !strings.Contains(lines[i], text) {
			t.Fatalf("popup line %d = %q, want %q", i, lines[i], text)
		}
		if !strings.Contains(question, text) {
			t.Fatalf("question %q lacks %q", question, text)
		}
	}
}

// With several sessions asking at once, the approver tells requests apart
// by repository, Herdr workspace, session, and PR title, so the Herdr
// notification pde-gh starts for a new request must record the agent. Before this, the popup led with the request ID and the
// phone said only "current repository", so the Herdr workspace and agent
// title are what now tell the approver which of several agents asked.
func TestRequestRecordsHerdrAgentContext(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake herdr lists one agent for session ses_1 and its workspace, and
	// touches done when the notification is shown.
	done := filepath.Join(t.TempDir(), "done")
	approvaltest.FakeHerdr(t, `case "$1 $2" in
"agent list") echo '{"result":{"agents":[{"agent_session":{"value":"ses_1"},"pane_id":"w1:p2","workspace_id":"w1","terminal_title":"OC | release"}]}}' ;;
"workspace list") echo '{"result":{"workspaces":[{"workspace_id":"w1","label":"pdev-release"}]}}' ;;
"notification show") touch '`+done+`' ;;
esac`)
	// HERDR_ENV turns the Herdr calls on.
	t.Setenv("HERDR_ENV", "1")
	saveRequest(t, approval.Record{ID: "ctx", State: approval.StatePending, SessionID: "ses_1"})

	notifyHerdr("ctx")

	got, err := approval.Load("ctx")
	if err != nil {
		t.Fatal(err)
	}
	if got.PaneID != "w1:p2" || got.Workspace != "pdev-release" || got.SessionTitle != "OC | release" {
		t.Fatalf("context = %#v", got)
	}
	waitForFile(t, done)
}

// OpenCode runs every agent's tools under one shared server, so a
// subagent's shell inherits the server's HERDR_PANE_ID: the pane the server
// started in, which belongs to another agent. Using that pane once labeled a
// merge request with another workspace's agent, and the owner declined it as
// suspicious. A subagent Herdr does not list must show its session ID instead.
func TestSubagentRequestNeverNamesServerPane(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake herdr lists only the main agent that started the server, in
	// pane w1:p2, answers pane get for that pane, and touches done when the
	// notification is shown.
	done := filepath.Join(t.TempDir(), "done")
	approvaltest.FakeHerdr(t, `case "$1 $2" in
"agent list") echo '{"result":{"agents":[{"agent_session":{"value":"ses_main"},"pane_id":"w1:p2","workspace_id":"w1","terminal_title":"OC | other work"}]}}' ;;
"pane get") echo '{"result":{"pane":{"pane_id":"w1:p2","workspace_id":"w1","terminal_title":"OC | other work"}}}' ;;
"workspace list") echo '{"result":{"workspaces":[{"workspace_id":"w1","label":"other-repo"}]}}' ;;
"notification show") touch '`+done+`' ;;
esac`)
	t.Setenv("HERDR_ENV", "1")
	// Inherited from the shared server, not set for this agent.
	t.Setenv("HERDR_PANE_ID", "w1:p2")
	saveRequest(t, approval.Record{ID: "sub", State: approval.StatePending, SessionID: "ses_subagent", Repo: "owner/repo", Args: []string{"pr", "merge", "185"}})

	notifyHerdr("sub")

	got, err := approval.Load("sub")
	if err != nil {
		t.Fatal(err)
	}
	waitForFile(t, done)

	if got.PaneID != "" || got.Workspace != "" || got.SessionTitle != "" {
		t.Fatalf("context = %#v", got)
	}
	if lines := approval.ContextLines(got); len(lines) < 3 || lines[2] != "ses_subagent" {
		t.Fatalf("context lines = %q", lines)
	}
}

// Saves rec as the request pde-gh would leave waiting.
func saveRequest(t *testing.T, rec approval.Record) {
	t.Helper()
	if err := approval.Save(rec); err != nil {
		t.Fatal(err)
	}
}

func waitForFile(t *testing.T, path string) {
	t.Helper()
	for deadline := time.Now().Add(5 * time.Second); time.Now().Before(deadline); time.Sleep(10 * time.Millisecond) {
		if _, err := os.Stat(path); err == nil {
			return
		}
	}
	t.Fatalf("%s never appeared", path)
}

// The Herdr notification runs beside the wait for an answer, so the answer
// can arrive while the "Approval waiting" marker is still being set. A
// marker set after the answer would stay on the agent's pane, though nothing
// waits there any more.
func TestMarkerClearedWhenAnswerArrivesFirst(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	dir := t.TempDir()
	// The fake herdr touches waiting and then holds the "Approval waiting"
	// marker call until the gate file exists, so the answer below lands while
	// the notification is mid-way. It logs marker calls and touches done
	// when it shows the notification.
	gate, waiting, done := filepath.Join(dir, "gate"), filepath.Join(dir, "waiting"), filepath.Join(dir, "done")
	log := approvaltest.FakeHerdr(t, `case "$1 $2" in
"agent list") echo '{"result":{"agents":[{"agent_session":{"value":"ses_1"},"pane_id":"w1:p2","workspace_id":"w1","terminal_title":"OC | release"}]}}' ;;
"workspace list") echo '{"result":{"workspaces":[{"workspace_id":"w1","label":"pdev-release"}]}}' ;;
"pane report-metadata")
  case "$*" in *--clear-title*) ;; *) touch '`+waiting+`'; while [ ! -e '`+gate+`' ]; do sleep 0.01; done ;; esac
  echo "$*" >> "$log" ;;
"notification show") touch '`+done+`' ;;
esac`)
	t.Setenv("HERDR_ENV", "1")
	saveRequest(t, approval.Record{ID: "race", State: approval.StatePending, SessionID: "ses_1"})

	// Start the notification, and approve the request while it waits on the
	// gate.
	go notifyHerdr("race")
	waitForFile(t, waiting)
	if code := saveAnswer("race", approval.ChoiceYes); code != 0 {
		t.Fatalf("saveAnswer returned %d", code)
	}
	// Release the notification and wait for it to finish.
	if err := os.WriteFile(gate, nil, 0o600); err != nil {
		t.Fatal(err)
	}
	markers := waitForNotification(t, log, done)

	if last := markers[len(markers)-1]; !strings.Contains(last, "--clear-title") {
		t.Fatalf("last marker operation = %q", last)
	}
}

// Returns the marker operations once the notification either cleared its own
// late marker or went on to show the notification.
func waitForNotification(t *testing.T, log, done string) []string {
	t.Helper()
	for deadline := time.Now().Add(5 * time.Second); time.Now().Before(deadline); time.Sleep(10 * time.Millisecond) {
		data, _ := os.ReadFile(log)
		_, afterSet, set := strings.Cut(string(data), "--title Approval waiting")
		if _, err := os.Stat(done); err == nil || (set && strings.Contains(afterSet, "--clear-title")) {
			return strings.Split(strings.TrimSpace(string(data)), "\n")
		}
	}
	t.Fatal("notification did not finish")
	return nil
}
