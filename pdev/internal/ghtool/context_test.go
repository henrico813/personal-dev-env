package ghtool

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pdev/internal/approval"
	"pdev/internal/approvaltest"
)

// Users approved other sessions' merges without knowing what they were;
// a merge request names only a number, so the looked-up title must show.
func TestMergeShowsLookedUpPRTitle(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// The fake gh answers the repository lookup and the PR title lookup.
	approvaltest.FakeGH(t, approvaltest.GHAnswers("repo view", "owner/repo"),
		approvaltest.GHAnswers("pr view", "Ship it"))
	discardStderr(t)

	rec := newRequest(t, []string{"pr", "merge", "185", "--squash"})

	if lines := approval.ContextLines(rec); rec.PRTitle != "Ship it" || !strings.Contains(strings.Join(lines, "\n"), "Ship it") {
		t.Fatalf("PR title %q, context lines %q", rec.PRTitle, lines)
	}
}

// A bare directory name shown where owner/repo goes reads like a real
// repository; a failed PR lookup must not invent a title either.
func TestUnknownRepoShowsDirectoryLabel(t *testing.T) {
	t.Setenv("XDG_STATE_HOME", t.TempDir())
	// Every gh lookup fails, as outside a repository or without network.
	approvaltest.FakeGH(t, "exit 1")
	project := filepath.Join(t.TempDir(), "project")
	if err := os.Mkdir(project, 0o700); err != nil {
		t.Fatal(err)
	}
	chdir(t, project)
	discardStderr(t)

	rec := newRequest(t, []string{"pr", "merge", "185"})

	if got := strings.Join(approval.ContextLines(rec), "\n"); got != "merge #185\ndir: project" {
		t.Fatalf("context = %q", got)
	}
}
