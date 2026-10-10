package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestPopupAndPhoneShowSameContext(t *testing.T) {
	// With several sessions asking at once, the first lines are how a person
	// tells requests apart, on the popup and on the phone alike.
	rec := record{Args: []string{"pr", "merge", "185"}, Repo: "owner/repo",
		Workspace: "pdev-release", SessionTitle: "OC | release", Title: "Ship it"}

	lines, _ := popupScreen(popupView{rec: rec, queue: "1 of 2"}, 37, 14)
	question := moshiQuestion(rec)

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

func TestRequestRecordsHerdrAgentContext(t *testing.T) {
	// With several sessions asking at once, the approver tells requests apart
	// by repository, Herdr workspace, session, and PR title, so a new request
	// must record them.
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake herdr lists one agent for session ses_1 and its workspace, and
	// touches done when the notification is shown.
	dir := t.TempDir()
	done := filepath.Join(dir, "done")
	writeScript(t, dir, "herdr", `case "$1 $2" in
"agent list") echo '{"result":{"agents":[{"agent_session":{"value":"ses_1"},"pane_id":"w1:p2","workspace_id":"w1","terminal_title":"OC | release"}]}}' ;;
"workspace list") echo '{"result":{"workspaces":[{"workspace_id":"w1","label":"pdev-release"}]}}' ;;
"notification show") touch '`+done+`' ;;
esac`)
	// The fake gh answers the repository and current-PR lookups.
	gh := writeScript(t, dir, "gh", `case "$1 $2" in "repo view") echo owner/repo ;; "pr view") echo '{"title":"Ship it","body":"old"}' ;; esac`)
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	// The fake herdr comes first on PATH; HERDR_ENV turns the Herdr calls on.
	t.Setenv("HERDR_ENV", "1")
	t.Setenv("OPENCODE_SESSION_ID", "ses_1")
	discardStderr(t)
	req, err := parse([]string{"pr", "edit", "185", "--body", "new"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	req = scopeRequest(req)

	if _, err := loadOrCreate(req, gh); err != nil {
		t.Fatal(err)
	}
	// The notification runs in the background; it must not outlive the test's PATH.
	waitForFile(t, done)

	got, err := load(req.ID)
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

func TestAnswerSurvivesConcurrentNotification(t *testing.T) {
	// Herdr notification runs beside the wait for an answer. A stray save there
	// once could put an approved request back to pending, and a late marker
	// could leave "Approval waiting" on a pane after the answer.
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	dir := t.TempDir()
	// The fake herdr holds the "Approval waiting" marker call until the gate
	// file exists, so the answer below lands while the notification is
	// mid-way. It logs marker calls and touches done when it shows the
	// notification.
	gate, done, log := filepath.Join(dir, "gate"), filepath.Join(dir, "done"), filepath.Join(dir, "log")
	writeScript(t, dir, "herdr", `case "$1 $2" in
"agent list") echo '{"result":{"agents":[{"agent_session":{"value":"ses_1"},"pane_id":"w1:p2","workspace_id":"w1","terminal_title":"OC | release"}]}}' ;;
"workspace list") echo '{"result":{"workspaces":[{"workspace_id":"w1","label":"pdev-release"}]}}' ;;
"pane report-metadata")
  case "$*" in *--clear-title*) ;; *) while [ ! -e '`+gate+`' ]; do sleep 0.01; done ;; esac
  echo "$*" >> '`+log+`' ;;
"notification show") touch '`+done+`' ;;
esac`)
	gh := writeScript(t, dir, "gh", `case "$1 $2" in "repo view") echo owner/repo ;; "pr view") echo '{"title":"Ship it","body":"old"}' ;; esac`)
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))
	t.Setenv("HERDR_ENV", "1")
	t.Setenv("OPENCODE_SESSION_ID", "ses_1")
	discardStderr(t)
	req, err := parse([]string{"pr", "edit", "185", "--body", "new"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	req = scopeRequest(req)

	// Create the request, which starts the notification in the background,
	// and approve it while the notification waits on the gate.
	if state, err := loadOrCreate(req, gh); err != nil || state != statePending {
		t.Fatalf("state %q, err %v", state, err)
	}
	if code := saveAnswer(req.ID, choiceYes); code != 0 {
		t.Fatalf("saveAnswer returned %d", code)
	}
	// Release the notification and wait for it to finish.
	if err := os.WriteFile(gate, nil, 0o600); err != nil {
		t.Fatal(err)
	}
	markers := waitForNotification(t, log, done)

	got, err := load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got.State != stateApproved {
		t.Fatalf("state = %q after notification", got.State)
	}
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

func TestMergeShowsLookedUpPRTitle(t *testing.T) {
	// Users approved other sessions' merges without knowing what they were;
	// a merge request names only a number, so the looked-up title must show.
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake gh answers the repository lookup and the PR title lookup.
	gh := writeScript(t, t.TempDir(), "gh", `case "$1 $2" in "repo view") echo owner/repo ;; "pr view") echo 'Ship it' ;; esac`)
	discardStderr(t)
	req, err := parse([]string{"pr", "merge", "185", "--squash"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	req = scopeRequest(req)

	if _, err := loadOrCreate(req, gh); err != nil {
		t.Fatal(err)
	}

	rec, err := load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	lines, _ := popupScreen(popupView{rec: rec, queue: "1 of 1"}, 37, 14)
	if rec.PRTitle != "Ship it" || !strings.Contains(strings.Join(lines[:4], "\n"), "Ship it") {
		t.Fatalf("PR title %q, popup %q", rec.PRTitle, lines[:4])
	}
}

func TestUnknownRepoShowsDirectoryLabel(t *testing.T) {
	// A bare directory name shown where owner/repo goes reads like a real
	// repository; a failed PR lookup must not invent a title either.
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// Every gh lookup fails, as outside a repository or without network.
	gh := writeScript(t, t.TempDir(), "gh", "exit 1")
	project := filepath.Join(t.TempDir(), "project")
	if err := os.Mkdir(project, 0o700); err != nil {
		t.Fatal(err)
	}
	chdir(t, project)
	discardStderr(t)
	req, err := parse([]string{"pr", "merge", "185"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	req = scopeRequest(req)

	if _, err := loadOrCreate(req, gh); err != nil {
		t.Fatal(err)
	}

	rec, err := load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got := strings.Join(requestContext(rec), "\n"); got != "merge #185\ndir: project" {
		t.Fatalf("context = %q", got)
	}
}
