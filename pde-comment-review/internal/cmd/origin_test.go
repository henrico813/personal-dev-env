package cmd

import (
	"os/exec"
	"path/filepath"
	"testing"
)

// The owner once answered the wrong review because nothing named its repo.
// A worktree's folder, or a main checkout's relative ".git", would name it
// wrongly.
func TestLabelNamesMainRepoAndBranch(t *testing.T) {
	// Keeps the owner's git config and a live Herdr out of the test.
	t.Setenv("GIT_CONFIG_GLOBAL", "/dev/null")
	t.Setenv("GIT_CONFIG_NOSYSTEM", "1")
	t.Setenv("HERDR_ENV", "")
	root := t.TempDir()
	repo := filepath.Join(root, "neurokit")
	worktree := filepath.Join(root, "nkit-003-broker-v2")
	git := func(args ...string) {
		t.Helper()
		args = append([]string{"-c", "user.name=Test", "-c", "user.email=test@example.com"}, args...)
		if output, err := exec.Command("git", args...).CombinedOutput(); err != nil {
			t.Fatalf("git %v: %v\n%s", args, err, output)
		}
	}
	git("init", "-q", "-b", "main", repo)
	git("-C", repo, "commit", "-q", "--allow-empty", "-m", "start")
	git("-C", repo, "worktree", "add", "-q", "-b", "vibe/nkit-003-broker-v2", worktree)
	tests := []struct {
		name string
		dir  string
		want string
	}{
		{"main-checkout", repo, "neurokit · main"},
		{"worktree", worktree, "neurokit · nkit-003-broker-v2"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got := findReviewOrigin(tt.dir).Label()

			if got != tt.want {
				t.Errorf("label = %q, want %q", got, tt.want)
			}
		})
	}
}
