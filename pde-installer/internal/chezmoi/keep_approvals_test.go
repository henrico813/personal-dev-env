package chezmoi

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// One merged pull request for the fake gh. The test replaces HEAD with
// the worktree's commit.
const mergedFromHead = `[{"number": 7, "headRefOid": "HEAD", "mergeCommit": {"oid": "abc123"}}]`

// Verifies that removing a worktree never loses the owner's approved comments.
// Every run makes a backup, and the removal stops when a merge fails or is
// unclear.
func TestKeepApprovalsNeverLosesApprovals(t *testing.T) {
	tests := map[string]struct {
		pullRequests string
		ghFails      bool
		noChecker    bool
		checkerExit  int
		wantOK       bool
		wantMerged   bool
		wantOutput   string
	}{
		"merged pull request": {
			pullRequests: mergedFromHead,
			wantOK:       true,
			wantMerged:   true,
		},
		"no merged pull request": {
			pullRequests: `[]`,
			wantOK:       true,
		},
		// GitHub's Update branch button moves the head to a commit the
		// worktree lacks.
		"merged from another commit": {
			pullRequests: strings.Replace(
				mergedFromHead,
				"HEAD",
				"0000",
				1,
			),
			wantOutput: "#7",
		},
		// Offline, or signed out of GitHub.
		"gh fails": {
			ghFails:    true,
			wantOutput: "gh could not list",
		},
		"merge refused": {
			pullRequests: mergedFromHead,
			checkerExit:  1,
			wantMerged:   true,
		},
		// The checker is installed only on the full profile.
		"checker not installed": {
			pullRequests: mergedFromHead,
			noChecker:    true,
			wantOK:       true,
			wantOutput:   "pde-comment-checker is not installed",
		},
	}
	script := filepath.Join(
		repoRoot(t),
		"chezmoi",
		"dot_local",
		"bin",
		"executable_pde-keep-approvals",
	)
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			if _, err := exec.LookPath("jq"); err != nil {
				t.Skip("the fake gh needs jq")
			}
			root, err := filepath.EvalSymlinks(t.TempDir())
			if err != nil {
				t.Fatal(err)
			}
			main := filepath.Join(
				root,
				"main",
			)
			worktree := filepath.Join(
				root,
				"worktree",
			)
			seedBranchCheckRepo(
				t,
				main,
			)
			runGit(
				t,
				main,
				"worktree",
				"add",
				"--quiet",
				"-b",
				"feat",
				worktree,
			)
			for _, dir := range []string{
				main,
				worktree,
			} {
				writeApplyFile(
					t,
					filepath.Join(
						dir,
						".code-comments.json",
					),
					`{"approved": [], "avoid": {}}`,
				)
			}
			head, err := exec.Command(
				"git",
				"-C",
				worktree,
				"rev-parse",
				"HEAD",
			).Output()
			if err != nil {
				t.Fatal(err)
			}
			bin := filepath.Join(
				root,
				"bin",
			)
			fakeGh := "#!/bin/sh\nexit 1\n"
			if !test.ghFails {
				fakeGh = "#!/bin/sh\n" +
					"while [ $# -gt 0 ]; do\n" +
					"    if [ \"$1\" = --jq ]; then filter=$2; fi\n" +
					"    shift\n" +
					"done\n" +
					"jq -r \"$filter\" <<'EOF'\n" +
					strings.ReplaceAll(
						test.pullRequests,
						"HEAD",
						strings.TrimSpace(string(head)),
					) +
					"\nEOF\n"
			}
			writeExecutable(
				t,
				filepath.Join(
					bin,
					"gh",
				),
				fakeGh,
			)
			calls := filepath.Join(
				root,
				"checker-calls",
			)
			if !test.noChecker {
				writeExecutable(
					t,
					filepath.Join(
						bin,
						"pde-comment-checker",
					),
					fmt.Sprintf(
						"#!/bin/sh\necho \"$@\" > %s\nexit %d\n",
						calls,
						test.checkerExit,
					),
				)
			}
			state := filepath.Join(
				root,
				"state",
			)

			command := exec.Command(
				"bash",
				script,
				worktree,
			)
			command.Env = append(
				isolatedGitEnv(os.Environ()),
				"PATH="+bin+string(os.PathListSeparator)+removeCheckerFromPath(),
				"XDG_STATE_HOME="+state,
			)
			output, err := command.CombinedOutput()

			if gotOK := err == nil; gotOK != test.wantOK {
				t.Errorf(
					"pde-keep-approvals passed = %t, want %t:\n%s",
					gotOK,
					test.wantOK,
					output,
				)
			}
			if !strings.Contains(
				string(output),
				test.wantOutput,
			) {
				t.Errorf(
					"output = %s, want it to contain %q",
					output,
					test.wantOutput,
				)
			}
			for _, record := range []string{
				"worktree/.code-comments.json",
				".code-comments.json",
			} {
				backups, err := filepath.Glob(filepath.Join(
					state,
					"pde",
					"approval-backups",
					"*",
					record,
				))
				if err != nil || len(backups) != 1 {
					t.Errorf(
						"backups of %s = %v, %v; want one",
						record,
						backups,
						err,
					)
				}
			}
			wantCall := ""
			if test.wantMerged {
				wantCall = "--repo " + main + " --merge-approvals " + worktree + " abc123\n"
			}
			gotCall, _ := os.ReadFile(calls)
			if string(gotCall) != wantCall {
				t.Errorf(
					"checker was run with %q, want %q",
					gotCall,
					wantCall,
				)
			}
		})
	}
}

// Returns PATH without the folders that hold an installed
// pde-comment-checker, so only the test's fake can run.
func removeCheckerFromPath() string {
	var kept []string
	for _, dir := range filepath.SplitList(os.Getenv("PATH")) {
		_, err := os.Stat(filepath.Join(
			dir,
			"pde-comment-checker",
		))
		if err != nil {
			kept = append(
				kept,
				dir,
			)
		}
	}
	return strings.Join(
		kept,
		string(os.PathListSeparator),
	)
}
