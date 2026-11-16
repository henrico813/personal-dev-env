package cmd

import (
	"os"
	"path/filepath"
	"strconv"
	"testing"

	"pdev/internal/approvaltest"
)

// Agents write PRs only through pde-gh-write, which asks the user first. A
// write the guard lets through skips that approval, and a read it refuses
// breaks the agent's normal gh use.
func TestFindRefusalCoversWritesAndReads(t *testing.T) {
	tests := []struct {
		name string
		args []string
		want string
	}{
		{
			name: "pr create",
			args: []string{"pr", "create", "--fill"},
			want: prWriteMessage,
		},
		{
			name: "pr edit",
			args: []string{"pr", "edit", "7"},
			want: prWriteMessage,
		},
		{
			name: "pr comment",
			args: []string{"pr", "comment", "7"},
			want: prWriteMessage,
		},
		{
			name: "pr review",
			args: []string{"pr", "review", "7"},
			want: prWriteMessage,
		},
		{
			name: "pr merge",
			args: []string{"pr", "merge", "7"},
			want: prWriteMessage,
		},
		{
			name: "pr ready",
			args: []string{"pr", "ready", "7"},
			want: prWriteMessage,
		},
		{
			name: "pr close",
			args: []string{"pr", "close", "7"},
			want: prWriteMessage,
		},
		{
			name: "pr reopen",
			args: []string{"pr", "reopen", "7"},
			want: prWriteMessage,
		},
		{
			name: "pr new",
			args: []string{"pr", "new"},
			want: prNewMessage,
		},
		{
			name: "api -X POST",
			args: []string{"api", "-X", "POST", "repos/o/r/pulls"},
			want: apiMessage,
		},
		{
			name: "api --method patch",
			args: []string{"api", "--method", "patch", "repos/o/r/pulls/7"},
			want: apiMessage,
		},
		{
			name: "api -X=PUT",
			args: []string{"api", "-X=PUT", "repos/o/r/pulls/7/merge"},
			want: apiMessage,
		},
		{
			name: "api --method=DELETE",
			args: []string{"api", "--method=DELETE", "repos/o/r/issues/comments/9"},
			want: apiMessage,
		},
		{
			name: "api -f",
			args: []string{"api", "repos/o/r/issues/7/comments", "-f", "body=hi"},
			want: apiMessage,
		},
		{
			name: "api -F",
			args: []string{"api", "repos/o/r/pulls", "-F", "draft=true"},
			want: apiMessage,
		},
		{
			name: "api --field",
			args: []string{"api", "repos/o/r/pulls", "--field", "title=x"},
			want: apiMessage,
		},
		{
			name: "api --raw-field",
			args: []string{"api", "repos/o/r/pulls", "--raw-field", "title=x"},
			want: apiMessage,
		},
		{
			name: "api --input",
			args: []string{"api", "repos/o/r/pulls", "--input", "body.json"},
			want: apiMessage,
		},
		{
			name: "api -f=",
			args: []string{"api", "repos/o/r/pulls", "-f=title=x"},
			want: apiMessage,
		},
		{
			name: "api --input=",
			args: []string{"api", "repos/o/r/pulls", "--input=body.json"},
			want: apiMessage,
		},
		{
			name: "graphql mutation",
			args: []string{"api", "graphql", "-f", "query=mutation { mergePullRequest }"},
			want: apiMessage,
		},
		{
			name: "pr list",
			args: []string{"pr", "list", "--state", "all"},
			want: "",
		},
		{
			name: "pr view",
			args: []string{"pr", "view", "7", "--json", "state"},
			want: "",
		},
		{
			name: "api pulls read",
			args: []string{"api", "repos/o/r/pulls/7"},
			want: "",
		},
		{
			name: "api pulls get with flag",
			args: []string{"api", "repos/o/r/pulls", "-X", "GET", "--paginate"},
			want: "",
		},
		{
			name: "api issue field",
			args: []string{"api", "repos/o/r/issues", "-f", "title=x"},
			want: "",
		},
		{
			name: "graphql query",
			args: []string{"api", "graphql", "-f", "query={ viewer { login } }"},
			want: "",
		},
		{
			name: "repo view",
			args: []string{"repo", "view"},
			want: "",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			if got := findRefusal(tt.args); got != tt.want {
				t.Errorf("findRefusal(%q) = %q, want %q", tt.args, got, tt.want)
			}
		})
	}
}

// A refused write must not reach gh and must exit 2, the code agents and
// docs treat as a refusal rather than a gh failure.
func TestGuardRefusesWriteWithoutRunningGH(t *testing.T) {
	_, log := approvaltest.FakeGH(t, approvaltest.GHLogsCall)
	t.Setenv("PDE_GH_WRITE", "")
	discardStderr(t)

	code := guardGH([]string{"pr", "merge", "7"})

	if code != exitRejected {
		t.Errorf("guardGH() = %d, want %d", code, exitRejected)
	}
	if calls := countCalls(t, log); calls != 0 {
		t.Errorf("gh ran %d times, want 0", calls)
	}
}

// pde-gh-write runs an approved write through the guard with PDE_GH_WRITE=1.
// If the guard refused it, no approved write could ever run.
func TestGuardRunsWriteWhenApproved(t *testing.T) {
	_, log := approvaltest.FakeGH(t, approvaltest.GHLogsCall, "exit 5")
	t.Setenv("PDE_GH_WRITE", "1")

	code := guardGH([]string{"pr", "merge", "7"})

	if code != 5 {
		t.Errorf("guardGH() = %d, want gh's exit code 5", code)
	}
	if calls := countCalls(t, log); calls != 1 {
		t.Errorf("gh ran %d times, want 1", calls)
	}
}

// aqua's shim with no config runs the first gh on PATH, the guard again. The
// guard must stop there, or the pair loops forever and hangs timed jobs.
func TestGuardStopsLoopThroughShim(t *testing.T) {
	_, log := approvaltest.FakeGH(t, approvaltest.GHLogsCall)
	t.Setenv(guardPIDVariable, strconv.Itoa(os.Getppid()))
	discardStderr(t)

	code := guardGH([]string{"pr", "list"})

	if code != exitFailed {
		t.Errorf("guardGH() = %d, want %d", code, exitFailed)
	}
	if calls := countCalls(t, log); calls != 0 {
		t.Errorf("gh ran %d times, want 0", calls)
	}
}

// Timed jobs run without aqua's config, so the guard must reach aqua's gh
// without the shim. aqua keeps old versions, and the newest is the one in use.
func TestFindRealGHPicksNewestAqua(t *testing.T) {
	root := t.TempDir()
	proxyDir := filepath.Join(root, "internal", "aqua-proxy")
	for _, dir := range []string{filepath.Join(root, "bin"), proxyDir} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	proxy := approvaltest.WriteScript(t, proxyDir, "aqua-proxy", "exit 99")
	if err := os.Symlink(proxy, filepath.Join(root, "bin", "gh")); err != nil {
		t.Fatal(err)
	}
	cli := filepath.Join(root, "pkgs", "github_release", "github.com", "cli", "cli")
	for _, version := range []string{"v2.94.0", "v2.101.0"} {
		bin := filepath.Join(cli, version, "gh.tar.gz", "gh", "bin")
		if err := os.MkdirAll(bin, 0o755); err != nil {
			t.Fatal(err)
		}
		approvaltest.WriteScript(t, bin, "gh", "exit 0")
	}
	t.Setenv("PATH", filepath.Join(root, "bin"))

	gh := findRealGH()

	if want := filepath.Join(cli, "v2.101.0", "gh.tar.gz", "gh", "bin", "gh"); gh != want {
		t.Errorf("findRealGH() = %q, want %q", gh, want)
	}
}
