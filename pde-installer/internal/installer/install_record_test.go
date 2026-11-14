package installer

import (
	"bytes"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"pde-installer/internal/run"
)

// Each machine reports the build it runs, so the owner no longer compares
// commits by hand. A wrong field would point them at the wrong build.
func TestVersionPrintsRecordedInstall(t *testing.T) {
	home := t.TempDir()
	t.Setenv(
		"HOME",
		home,
	)
	repo := initTestCheckout(
		t,
		"feature/demo",
	)
	commit := runTestGit(
		t,
		repo,
		"rev-parse",
		"HEAD",
	)
	installedAt := time.Date(
		2026,
		11,
		10,
		9,
		30,
		0,
		0,
		time.FixedZone(
			"CET",
			3600,
		),
	)
	recordInstall(
		config{
			Home:     home,
			RepoRoot: repo,
		},
		run.Runner{},
		installedAt,
	)

	var stdout bytes.Buffer
	command := NewCommand()
	command.SetOut(&stdout)
	command.SetArgs([]string{"version"})
	err := command.Execute()

	if err != nil {
		t.Fatal(err)
	}
	want := "commit " + commit + "\nbranch feature/demo\ninstalled 2026-11-10T08:30:00Z\n" +
		"no update recorded\nmain not compared: no checkout found\n"
	if stdout.String() != want {
		t.Fatalf(
			"version output = %q, want %q",
			stdout.String(),
			want,
		)
	}
}

// A machine that was never installed must not print empty fields as if it
// ran some build.
func TestVersionFailsWithoutRecordedInstall(t *testing.T) {
	t.Setenv("HOME", t.TempDir())

	var stdout bytes.Buffer
	command := NewCommand()
	command.SetOut(&stdout)
	command.SetArgs([]string{"version"})
	err := command.Execute()

	want := "no install recorded on this machine; run pde-installer install first"
	if err == nil || err.Error() != want {
		t.Fatalf(
			"Execute() error = %v, want %q",
			err,
			want,
		)
	}
	if stdout.Len() != 0 {
		t.Fatalf(
			"version printed %q without a record",
			stdout.String(),
		)
	}
}

// The owner checks a PC with version instead of opening update.json. A
// checkout left on a branch must still show that main moved past the install.
func TestVersionReportsUpdateAndMainLead(t *testing.T) {
	home := t.TempDir()
	t.Setenv(
		"HOME",
		home,
	)
	repo := initTestCheckout(
		t,
		"main",
	)
	installed := runTestGit(
		t,
		repo,
		"rev-parse",
		"HEAD",
	)
	runTestGit(
		t,
		repo,
		"-c",
		"user.name=Test",
		"-c",
		"user.email=test@example.invalid",
		"commit",
		"--quiet",
		"--no-verify",
		"--allow-empty",
		"-m",
		"new main",
	)
	runTestGit(
		t,
		repo,
		"switch",
		"--quiet",
		"--create",
		"feature/demo",
		installed,
	)
	writeCommandTestFile(
		t,
		filepath.Join(
			home,
			".config",
			"pde",
			"config.json",
		),
		`{"install_path": "`+repo+`"}`,
	)
	if err := writeInstallRecord(
		home,
		installRecord{
			Commit:      installed,
			Branch:      "main",
			InstalledAt: "2026-11-13T08:00:00Z",
		},
	); err != nil {
		t.Fatal(err)
	}
	if err := writeStateJSON(
		home,
		updateRecordPath(home),
		updateRecord{
			Result:    "failed",
			Reason:    "install: apt broke",
			CheckedAt: "2026-11-13T08:30:00Z",
		},
	); err != nil {
		t.Fatal(err)
	}

	var stdout bytes.Buffer
	command := NewCommand()
	command.SetOut(&stdout)
	command.SetArgs([]string{"version"})
	err := command.Execute()

	if err != nil {
		t.Fatal(err)
	}
	want := "commit " + installed + "\nbranch main\ninstalled 2026-11-13T08:00:00Z\n" +
		"last update failed at 2026-11-13T08:30:00Z: install: apt broke\n" +
		"main is 1 commit ahead of the install\n"
	if stdout.String() != want {
		t.Fatalf(
			"version output = %q, want %q",
			stdout.String(),
			want,
		)
	}
}

// An install from main once replaced branch installs under test without a
// word, and the tests then ran the wrong code. The owner must see which branch
// install is replaced, and must not learn to ignore a warning on every install.
func TestWarnReplacingBranchOnlyFromMain(t *testing.T) {
	branchRecord := installRecord{
		Commit:      "3f9c2e1a7b",
		Branch:      "feat/x",
		InstalledAt: "2026-11-06T10:00:00Z",
	}
	mainRecord := installRecord{
		Commit:      "3f9c2e1a7b",
		Branch:      "main",
		InstalledAt: "2026-11-06T10:00:00Z",
	}
	tests := []struct {
		name           string
		checkoutBranch string
		record         *installRecord
		want           string
	}{
		{
			name:           "branch record",
			checkoutBranch: "main",
			record:         &branchRecord,
			want:           "warning: replacing the install from branch feat/x (commit 3f9c2e1a7b, installed 2026-11-06T10:00:00Z) with main\n",
		},
		{
			name:           "main record",
			checkoutBranch: "main",
			record:         &mainRecord,
			want:           "",
		},
		{
			name:           "no record",
			checkoutBranch: "main",
			record:         nil,
			want:           "",
		},
		{
			name:           "branch checkout",
			checkoutBranch: "feat/y",
			record:         &branchRecord,
			want:           "",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			home := t.TempDir()
			if tt.record != nil {
				if err := writeInstallRecord(
					home,
					*tt.record,
				); err != nil {
					t.Fatal(err)
				}
			}
			repo := initTestCheckout(
				t,
				tt.checkoutBranch,
			)
			var stderr bytes.Buffer

			warnReplacingBranch(
				config{
					Home:     home,
					RepoRoot: repo,
				},
				run.Runner{Stderr: &stderr},
			)

			if stderr.String() != tt.want {
				t.Fatalf(
					"warning = %q, want %q",
					stderr.String(),
					tt.want,
				)
			}
		})
	}
}

// A git checkout on branch with one empty commit, so rev-parse can name it.
func initTestCheckout(t *testing.T, branch string) string {
	t.Helper()
	t.Setenv(
		"GIT_CONFIG_GLOBAL",
		"/dev/null",
	)
	repo := t.TempDir()
	runTestGit(
		t,
		repo,
		"init",
		"--quiet",
		"--initial-branch",
		branch,
	)
	runTestGit(
		t,
		repo,
		"-c",
		"user.name=Test",
		"-c",
		"user.email=test@example.invalid",
		"commit",
		"--quiet",
		"--no-verify",
		"--allow-empty",
		"-m",
		"seed",
	)
	return repo
}

// Runs git in dir and returns its trimmed output.
func runTestGit(t *testing.T, dir string, args ...string) string {
	t.Helper()
	command := exec.Command(
		"git",
		append(
			[]string{
				"-C",
				dir,
			},
			args...,
		)...,
	)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf(
			"git %v failed: %v\n%s",
			args,
			err,
			output,
		)
	}
	return strings.TrimSpace(string(output))
}
