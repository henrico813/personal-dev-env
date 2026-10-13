package chezmoi

import (
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"testing"
)

// Agents may write to pull requests only through pde-gh-write, which asks the
// owner first. Most rows once reached the real gh unapproved, because the guard
// listed only some writes and did not read options the way gh does.
func TestGuardBlocksWritesWithoutApproval(t *testing.T) {
	tests := []struct {
		name     string
		give     []string
		wantHint string // Part of the message that tells the agent what to do.
	}{
		{"repo option before pr", []string{"-R", "owner/repo", "pr", "merge", "12"}, "Use pde-gh-write"},
		// gh skips empty arguments when it finds the command, so this is pr merge.
		{"empty word before subcommand", []string{"pr", "", "merge", "12"}, "Use pde-gh-write"},
		{"pr update-branch", []string{"pr", "update-branch", "12"}, "ask the user"},
		{"pr lock", []string{"pr", "lock", "12"}, "ask the user"},
		{"issue comment on a PR number", []string{"issue", "comment", "12", "--body", "hi"}, "ask the user"},
		{"joined method", []string{"api", "-XPOST", "repos/owner/repo/pulls/12/reviews"}, "ask the user"},
		{"long method", []string{"api", "--method=PUT", "repos/owner/repo/pulls/12/merge"}, "ask the user"},
		{"header before method", []string{"api", "-H", "Accept:x", "-X", "PUT", "repos/owner/repo/pulls/12/merge"}, "ask the user"},
		{"field without method", []string{"api", "repos/owner/repo/issues/12/comments", "-f", "body=hi"}, "ask the user"},
		{"body file", []string{"api", "repos/owner/repo/pulls/12/reviews", "--input", "review.json"}, "ask the user"},
		// gh takes -XGET as the template or jq text, so the write is still a POST.
		{"template value like GET", []string{"api", "-t", "-XGET", "repos/owner/repo/issues/12/comments", "-f", "body=hi"}, "ask the user"},
		{"jq value like GET", []string{"api", "--jq", "-XGET", "repos/owner/repo/issues/12/comments", "-f", "body=hi"}, "ask the user"},
		{"graphql query from file", []string{"api", "graphql", "-F", "query=@resolve-thread.graphql"}, "use a REST GET"},
		{"alias set", []string{"alias", "set", "ship", "pr merge --squash"}, "ask the user"},
		{"alias import", []string{"alias", "import", "aliases.yml"}, "ask the user"},
		{"agent create", []string{"agent", "create", "fix the tests"}, "ask the user"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			exit, stderr, realArgs := runGuard(t, tt.give...)

			if exit != 2 || realArgs != nil {
				t.Fatalf("gh %q: exit %d, real gh got %q; want exit 2 and no real gh", tt.give, exit, realArgs)
			}
			if !strings.Contains(stderr, tt.wantHint) {
				t.Errorf("gh %q: message %q does not contain %q", tt.give, stderr, tt.wantHint)
			}
		})
	}
}

// Agents read PRs all the time, so the guard must pass reads to the real gh
// with their arguments unchanged.
func TestGuardPassesReadsToRealGh(t *testing.T) {
	tests := []struct {
		name string
		give []string
	}{
		{"pr view", []string{"pr", "view", "12", "--json", "title"}},
		{"pr checks with repo", []string{"pr", "checks", "12", "-R", "owner/repo"}},
		{"issue view", []string{"issue", "view", "12"}},
		{"api GET", []string{"api", "repos/owner/repo/pulls/12/comments", "--paginate", "--jq", ".[].body"}},
		{"api GET with query fields", []string{"api", "-X", "GET", "search/issues", "-f", "q=repo:owner/repo is:open"}},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			exit, stderr, realArgs := runGuard(t, tt.give...)

			if exit != 0 || !slices.Equal(realArgs, tt.give) {
				t.Fatalf("gh %q: exit %d, real gh got %q, stderr %q; want exit 0 and the same arguments", tt.give, exit, realArgs, stderr)
			}
		})
	}
}

// runGuard runs the full-profile guard with a fake real gh after it on PATH.
// It returns the exit code, standard error, and the arguments the fake got,
// or nil when the guard did not run it.
func runGuard(t *testing.T, args ...string) (int, string, []string) {
	t.Helper()
	guardDir, realDir := t.TempDir(), t.TempDir()
	argsFile := filepath.Join(t.TempDir(), "real-gh-args")
	writeExecutable(t, filepath.Join(guardDir, "gh"), renderProfileTemplate(t, "dot_local/bin/executable_gh.tmpl", "full"))
	writeExecutable(t, filepath.Join(realDir, "gh"), "#!/bin/sh\nprintf '%s\\n' \"$@\" >'"+argsFile+"'\n")

	command := exec.Command(filepath.Join(guardDir, "gh"), args...)
	command.Env = append(os.Environ(), "PATH="+guardDir+":"+realDir+":"+os.Getenv("PATH"), "PDE_GH_WRITE=")
	var stderr strings.Builder
	command.Stderr = &stderr
	err := command.Run()
	exit := 0
	var exitErr *exec.ExitError
	if errors.As(err, &exitErr) {
		exit = exitErr.ExitCode()
	} else if err != nil {
		t.Fatal(err)
	}

	data, err := os.ReadFile(argsFile)
	if errors.Is(err, os.ErrNotExist) {
		return exit, stderr.String(), nil
	}
	if err != nil {
		t.Fatal(err)
	}
	return exit, stderr.String(), strings.Split(strings.TrimSuffix(string(data), "\n"), "\n")
}
