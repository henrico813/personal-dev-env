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
func TestSkillLessonsRenderNeverFailsInstall(t *testing.T) {
	tests := []struct {
		name       string
		script     string // The fake skill-lessons in ~/.local/bin, or "" for none.
		wantRender bool
		wantStderr string
	}{
		{
			name:       "installed only in home bin",
			script:     "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$HOME/calls\"\n",
			wantRender: true,
		},
		{
			name:       "render fails",
			script:     "#!/bin/sh\nprintf '%s\\n' \"$*\" > \"$HOME/calls\"\nexit 1\n",
			wantRender: true,
			wantStderr: "run skill-lessons render to restore lessons",
		},
		{
			name: "not installed",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			home := t.TempDir()
			t.Setenv("HOME", home)
			// Leave ~/.local/bin off PATH, as on a first install.
			t.Setenv("PATH", "/usr/bin:/bin")
			if tt.script != "" {
				binary := filepath.Join(home, ".local", "bin", "skill-lessons")
				writeCommandTestFile(t, binary, tt.script)
				if err := os.Chmod(binary, 0o755); err != nil {
					t.Fatal(err)
				}
			}
			var stderr bytes.Buffer
			runner := run.Runner{Stdout: &bytes.Buffer{}, Stderr: &stderr}

			err := renderSkillLessons(home, runner)

			if err != nil {
				t.Fatalf("render failed the install: %v", err)
			}
			calls, _ := os.ReadFile(filepath.Join(home, "calls"))
			if rendered := string(calls) == "render\n"; rendered != tt.wantRender {
				t.Errorf("render ran = %v, want %v; calls %q", rendered, tt.wantRender, calls)
			}
			if tt.wantStderr == "" && stderr.Len() != 0 {
				t.Errorf("stderr = %q, want none", stderr.String())
			}
			if !strings.Contains(stderr.String(), tt.wantStderr) {
				t.Errorf("stderr = %q, want substring %q", stderr.String(), tt.wantStderr)
			}
		})
	}
}
