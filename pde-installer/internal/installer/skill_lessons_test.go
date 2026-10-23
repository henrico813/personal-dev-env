package installer

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/run"
)

// Installing skills resets their Lessons sections, so a skipped render hides
// saved lessons, and a failed render must not fail the install.

func TestRenderRunsSkillLessonsOffPath(t *testing.T) {
	home := newHomeWithLocalBinOffPath(t, "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$HOME/calls\"\n")
	var stderr bytes.Buffer
	runner := run.Runner{Stdout: &bytes.Buffer{}, Stderr: &stderr}

	err := renderSkillLessons(home, runner)

	if err != nil {
		t.Fatalf("render failed the install: %v", err)
	}
	if calls := readSkillLessonsCalls(t, home); calls != "render\n" {
		t.Errorf("calls = %q, want render", calls)
	}
	if stderr.Len() != 0 {
		t.Errorf("stderr = %q, want none", stderr.String())
	}
}

func TestFailedSkillLessonsRenderOnlyWarns(t *testing.T) {
	home := newHomeWithLocalBinOffPath(t, "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$HOME/calls\"\nexit 1\n")
	var stderr bytes.Buffer
	runner := run.Runner{Stdout: &bytes.Buffer{}, Stderr: &stderr}

	err := renderSkillLessons(home, runner)

	if err != nil {
		t.Fatalf("render failed the install: %v", err)
	}
	if calls := readSkillLessonsCalls(t, home); calls != "render\n" {
		t.Errorf("calls = %q, want render", calls)
	}
	if want := "run skill-lessons render to restore lessons"; !strings.Contains(stderr.String(), want) {
		t.Errorf("stderr = %q, want substring %q", stderr.String(), want)
	}
}

func TestMissingSkillLessonsIsSkippedSilently(t *testing.T) {
	home := newHomeWithLocalBinOffPath(t, "")
	var stderr bytes.Buffer
	runner := run.Runner{Stdout: &bytes.Buffer{}, Stderr: &stderr}

	err := renderSkillLessons(home, runner)

	if err != nil {
		t.Fatalf("render failed the install: %v", err)
	}
	if calls := readSkillLessonsCalls(t, home); calls != "" {
		t.Errorf("calls = %q, want none", calls)
	}
	if stderr.Len() != 0 {
		t.Errorf("stderr = %q, want none", stderr.String())
	}
}

// Puts script, if any, at ~/.local/bin/skill-lessons in a new home whose
// .local/bin is off PATH, as on a first install.
func newHomeWithLocalBinOffPath(t *testing.T, script string) string {
	t.Helper()
	home := t.TempDir()
	t.Setenv("HOME", home)
	t.Setenv("PATH", "/usr/bin:/bin")
	if script != "" {
		binary := filepath.Join(home, ".local", "bin", "skill-lessons")
		writeCommandTestFile(t, binary, script)
		if err := os.Chmod(binary, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	return home
}

func readSkillLessonsCalls(t *testing.T, home string) string {
	t.Helper()
	calls, _ := os.ReadFile(filepath.Join(home, "calls"))
	return string(calls)
}
