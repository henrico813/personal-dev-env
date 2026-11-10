package cmd

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pdev2/internal/approval"
	"pdev2/internal/approvaltest"
)

// Agents titled skill changes docs, and a PR sent without the git-messages
// lint step went out unchecked. A refused title must stop before anyone is
// asked to approve it.
func TestDocsTitleRefusedBeforeApproval(t *testing.T) {
	env := setupFlow(t, "")
	fake := approvaltest.WriteScript(t, t.TempDir(), "msg", "echo 'line 1: docs refused'; exit 1")
	approvaltest.Replace(t, &msgCommand, []string{fake})

	code := requestWrite([]string{"pr", "create", "--title", "docs: add a rule"})

	if code != exitRejected {
		t.Errorf("docs title returned %d, want %d", code, exitRejected)
	}
	if queued := approval.Queued(); len(queued) != 0 {
		t.Errorf("%d requests waiting for approval, want 0", len(queued))
	}
	want := "pde-gh-write: refused the title before asking for approval: line 1: docs refused\n"
	if !strings.Contains(env.output.String(), want) {
		t.Errorf("output is missing %q:\n%s", want, env.output)
	}
}

// msg must judge the title by this PR's own files. pr edit may run from a
// checkout on another branch, and a stacked PR's branch also holds its
// parent's commits, so either way the checkout's branch would mislead msg.
func TestMsgGetsTitleAndPRScope(t *testing.T) {
	cases := []struct {
		name string
		args []string
		want string
	}{
		{
			name: "edit-sends-pr-files",
			args: []string{"pr", "edit", "12", "--title", "docs: add a rule"},
			want: "--path AGENTS.md",
		},
		{
			name: "create-sends-base",
			args: []string{"pr", "create", "--base", "parent", "--title", "docs: add a rule"},
			want: "--base parent",
		},
		{
			name: "create-sends-base-with-equals",
			args: []string{"pr", "create", "--base=parent", "--title", "docs: add a rule"},
			want: "--base parent",
		},
		{
			name: "create-sends-short-base",
			args: []string{"pr", "create", "-B", "parent", "--title", "docs: add a rule"},
			want: "--base parent",
		},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			env := setupFlow(t, "")
			approvaltest.RewriteFakeGH(t, env.gh,
				approvaltest.GHAnswers("pr diff", "AGENTS.md"),
				approvaltest.GHAnswers("repo view", "owner/repo"),
				approvaltest.GHAnswers("pr view", "title"),
				approvaltest.GHLogsCall)
			// The fake msg saves the title file and its arguments, then passes.
			seen := filepath.Join(t.TempDir(), "seen")
			fake := approvaltest.WriteScript(t, t.TempDir(), "msg",
				`cat "$1" > '`+seen+`'; echo "$@" >> '`+seen+`'`)
			approvaltest.Replace(t, &msgCommand, []string{fake})

			newRequest(t, tc.args)

			got, err := os.ReadFile(seen)
			if err != nil {
				t.Fatal(err)
			}
			if !strings.HasPrefix(string(got), "docs: add a rule\n") || !strings.Contains(string(got), tc.want) {
				t.Errorf("msg got %q, want the title and %q", got, tc.want)
			}
		})
	}
}

// A machine without the git-messages skill has no msg. PR writes must still
// reach the approver there, or every PR write would fail.
func TestMissingMsgStillAsksForApproval(t *testing.T) {
	env := setupFlow(t, "")
	approvaltest.Replace(t, &msgCommand, []string{filepath.Join(t.TempDir(), "msg")})

	rec := newRequest(t, []string{"pr", "create", "--title", "docs: add a rule"})

	if rec.Title != "docs: add a rule" {
		t.Errorf("waiting request has title %q", rec.Title)
	}
	if !strings.Contains(env.output.String(), "pde-gh-write: skipped the title type check: ") {
		t.Errorf("output does not say the check was skipped:\n%s", env.output)
	}
}
