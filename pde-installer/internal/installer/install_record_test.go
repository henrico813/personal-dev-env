package installer

import (
	"bytes"
	"os/exec"
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
		"feature/demo",
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
	want := "commit " + commit + "\nbranch feature/demo\ninstalled 2026-11-10T08:30:00Z\n"
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
