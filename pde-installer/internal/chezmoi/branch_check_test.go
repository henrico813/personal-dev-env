package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

// Verifies that pde-check-branch refuses badly named new branches but
// passes any existing branch, even one only on a remote, so people can
// still check out others' work.
func TestBranchCheckRefusesBadNewNames(t *testing.T) {
	tests := map[string]struct {
		pattern  string
		existing string
		branch   string
		wantOK   bool
	}{
		"type and words": {
			branch: "fix/abc-12-login-timeout",
			wantOK: true,
		},
		"unknown type": {
			branch: "bugfix/login-timeout",
			wantOK: false,
		},
		"old local branch": {
			existing: "refs/heads/bugfix/old/work",
			branch:   "bugfix/old/work",
			wantOK:   true,
		},
		"branch only on a remote": {
			existing: "refs/remotes/github/patch_1",
			branch:   "patch_1",
			wantOK:   true,
		},
		"repository pattern": {
			pattern: "^[A-Z]+-[0-9]+-[a-z0-9-]+$",
			branch:  "ABC-14-work",
			wantOK:  true,
		},
		"pattern replaces the default": {
			pattern: "^[A-Z]+-[0-9]+-[a-z0-9-]+$",
			branch:  "fix/login-timeout",
			wantOK:  false,
		},
	}
	script := filepath.Join(
		repoRoot(t),
		"chezmoi",
		"dot_local",
		"bin",
		"executable_pde-check-branch",
	)
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			repo := t.TempDir()
			seedBranchCheckRepo(
				t,
				repo,
			)
			if test.existing != "" {
				runGit(
					t,
					repo,
					"update-ref",
					test.existing,
					"HEAD",
				)
			}
			if test.pattern != "" {
				runGit(
					t,
					repo,
					"config",
					"pde.branchPattern",
					test.pattern,
				)
			}

			command := exec.Command(
				"bash",
				script,
				test.branch,
			)
			command.Dir = repo
			command.Env = isolatedGitEnv(os.Environ())
			output, err := command.CombinedOutput()

			if gotOK := err == nil; gotOK != test.wantOK {
				t.Errorf(
					"pde-check-branch %s passed = %t, want %t:\n%s",
					test.branch,
					gotOK,
					test.wantOK,
					output,
				)
			}
		})
	}
}

func seedBranchCheckRepo(t *testing.T, repo string) {
	t.Helper()
	runGit(
		t,
		".",
		"init",
		"--quiet",
		repo,
	)
	runGit(
		t,
		repo,
		"remote",
		"add",
		"github",
		"https://example.invalid/repo.git",
	)
	runGit(
		t,
		repo,
		"-c",
		"user.name=Test",
		"-c",
		"user.email=test@example.invalid",
		"commit",
		"--quiet",
		"--allow-empty",
		"-m",
		"seed",
	)
}

func runGit(t *testing.T, dir string, args ...string) {
	t.Helper()
	command := exec.Command(
		"git",
		args...,
	)
	command.Dir = dir
	command.Env = isolatedGitEnv(os.Environ())
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf(
			"git %s: %v\n%s",
			strings.Join(args, " "),
			err,
			output,
		)
	}
}

func isolatedGitEnv(env []string) []string {
	return append(
		env,
		"GIT_CONFIG_GLOBAL=/dev/null",
		"GIT_CONFIG_NOSYSTEM=1",
	)
}
