package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// With several sessions asking at once, the first lines are how a person
// tells requests apart. The phone used to ask "Approve gh pr merge in
// current repository (185)?" while the popup showed the command, so the two
// named a request differently and neither said which agent asked. Both now
// start with the same context lines, at the top of the popup.
func TestPopupAndPhoneShowSameContext(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	rec := record{ID: "ctx", State: statePending, Args: []string{"pr", "merge", "185"}, Repo: "owner/repo",
		Workspace: "pdev-release", SessionTitle: "OC | release", Title: "Ship it"}
	if err := writeRecord(rec); err != nil {
		t.Fatal(err)
	}
	// The fake moshi-hook saves the question it was asked, which comes after
	// "ask --require-remote --source pde-gh-write --timeout 4h", and declines.
	dir := t.TempDir()
	asked := filepath.Join(dir, "question")
	writeScript(t, dir, "moshi-hook", `printf '%s' "$7" > '`+asked+`'; exit 1`)
	t.Setenv("PATH", dir+":"+os.Getenv("PATH"))

	lines, _ := popupScreen(rec, "", "", 37, 14)
	moshiAsk([]string{rec.ID})

	sent, err := os.ReadFile(asked)
	if err != nil {
		t.Fatal(err)
	}
	question := string(sent)

	want := []string{"merge #185", "owner/repo", "pdev-release / OC | release", "Ship it"}
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
	// The fake herdr lists one agent for session ses_1 and its workspace. It
	// reports no socket, so the background popup request stops after asking
	// for one; it touches done then.
	dir := t.TempDir()
	done := filepath.Join(dir, "done")
	writeScript(t, dir, "herdr", `case "$1 $2" in
"agent list") echo '{"result":{"agents":[{"agent_session":{"value":"ses_1"},"pane_id":"w1:p2","workspace_id":"w1","terminal_title":"OC | release"}]}}' ;;
"workspace list") echo '{"result":{"workspaces":[{"workspace_id":"w1","label":"pdev-release"}]}}' ;;
"status server") touch '`+done+`' ;;
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
	// The popup request runs in the background; it must not outlive the test's PATH.
	waitForFile(t, done)

	got, err := load(req.ID)
	if err != nil {
		t.Fatal(err)
	}
	if got.Workspace != "pdev-release" || got.SessionTitle != "OC | release" ||
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

// Users approved other sessions' merges without knowing what they were;
// a merge request names only a number, so the looked-up title must show.
func TestMergeShowsLookedUpPRTitle(t *testing.T) {
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
	lines, _ := popupScreen(rec, "", "", 37, 14)
	if rec.PRTitle != "Ship it" || !strings.Contains(strings.Join(lines[:4], "\n"), "Ship it") {
		t.Fatalf("PR title %q, popup %q", rec.PRTitle, lines[:4])
	}
}

// A bare directory name shown where owner/repo goes reads like a real
// repository; a failed PR lookup must not invent a title either.
func TestUnknownRepoShowsDirectoryLabel(t *testing.T) {
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
