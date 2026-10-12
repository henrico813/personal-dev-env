package cmd

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
	"pdev2/internal/terminal"
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
// by repository, Herdr workspace, session, and PR title, so a new request
// must record them. Before this, the popup led with the request ID and the
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
	// The fake gh answers the repository and current-PR lookups.
	gh, _ := approvaltest.FakeGH(t, `case "$1 $2" in "repo view") echo owner/repo ;; "pr view") echo '{"title":"Ship it","body":"old"}' ;; esac`)
	// The fake herdr comes first on PATH; HERDR_ENV turns the Herdr calls on.
	t.Setenv("HERDR_ENV", "1")
	t.Setenv("OPENCODE_SESSION_ID", "ses_1")
	discardStderr(t)
	req, err := approval.Parse([]string{"pr", "edit", "185", "--body", "new"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	req = scopeRequest(req)

	if _, err := loadOrCreate(req, gh); err != nil {
		t.Fatal(err)
	}
	// The notification runs in the background; it must not outlive the test's PATH.
	waitForFile(t, done)

	got, err := approval.Load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got.PaneID != "w1:p2" || got.Workspace != "pdev-release" || got.SessionTitle != "OC | release" ||
		got.Repo != "owner/repo" || got.PRTitle != "Ship it" {
		t.Fatalf("context = %#v", got)
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
	// The fake herdr holds the "Approval waiting" marker call until the gate
	// file exists, so the answer below lands while the notification is
	// mid-way. It logs marker calls and touches done when it shows the
	// notification.
	gate, done := filepath.Join(dir, "gate"), filepath.Join(dir, "done")
	log := approvaltest.FakeHerdr(t, `case "$1 $2" in
"agent list") echo '{"result":{"agents":[{"agent_session":{"value":"ses_1"},"pane_id":"w1:p2","workspace_id":"w1","terminal_title":"OC | release"}]}}' ;;
"workspace list") echo '{"result":{"workspaces":[{"workspace_id":"w1","label":"pdev-release"}]}}' ;;
"pane report-metadata")
  case "$*" in *--clear-title*) ;; *) while [ ! -e '`+gate+`' ]; do sleep 0.01; done ;; esac
  echo "$*" >> "$log" ;;
"notification show") touch '`+done+`' ;;
esac`)
	gh, _ := approvaltest.FakeGH(t, `case "$1 $2" in "repo view") echo owner/repo ;; "pr view") echo '{"title":"Ship it","body":"old"}' ;; esac`)
	t.Setenv("HERDR_ENV", "1")
	t.Setenv("OPENCODE_SESSION_ID", "ses_1")
	discardStderr(t)
	req, err := approval.Parse([]string{"pr", "edit", "185", "--body", "new"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	req = scopeRequest(req)

	// Create the request, which starts the notification in the background,
	// and approve it while the notification waits on the gate.
	if state, err := loadOrCreate(req, gh); err != nil || state != approval.StatePending {
		t.Fatalf("state %q, err %v", state, err)
	}
	if code := saveAnswer(req.ID, approval.ChoiceYes); code != 0 {
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

// Users approved other sessions' merges without knowing what they were;
// a merge request names only a number, so the looked-up title must show.
func TestMergeShowsLookedUpPRTitle(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake gh answers the repository lookup and the PR title lookup.
	gh, _ := approvaltest.FakeGH(t, `case "$1 $2" in "repo view") echo owner/repo ;; "pr view") echo 'Ship it' ;; esac`)
	discardStderr(t)
	req, err := approval.Parse([]string{"pr", "merge", "185", "--squash"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	req = scopeRequest(req)

	if _, err := loadOrCreate(req, gh); err != nil {
		t.Fatal(err)
	}

	rec, err := approval.Load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	lines, _ := terminal.PopupScreen(terminal.View{Rec: rec, Queue: "1 of 1"}, 37, 14)
	if rec.PRTitle != "Ship it" || !strings.Contains(strings.Join(lines[:4], "\n"), "Ship it") {
		t.Fatalf("PR title %q, popup %q", rec.PRTitle, lines[:4])
	}
}

// A bare directory name shown where owner/repo goes reads like a real
// repository; a failed PR lookup must not invent a title either.
func TestUnknownRepoShowsDirectoryLabel(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// Every gh lookup fails, as outside a repository or without network.
	gh, _ := approvaltest.FakeGH(t, "exit 1")
	project := filepath.Join(t.TempDir(), "project")
	if err := os.Mkdir(project, 0o700); err != nil {
		t.Fatal(err)
	}
	chdir(t, project)
	discardStderr(t)
	req, err := approval.Parse([]string{"pr", "merge", "185"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	req = scopeRequest(req)

	if _, err := loadOrCreate(req, gh); err != nil {
		t.Fatal(err)
	}

	rec, err := approval.Load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got := strings.Join(approval.ContextLines(rec), "\n"); got != "merge #185\ndir: project" {
		t.Fatalf("context = %q", got)
	}
}
