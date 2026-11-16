package ghtool

import (
	"io"
	"strings"
	"testing"

	"pdev/internal/approval"
	"pdev/internal/approvaltest"
)

// The installer links gh, pde-gh-guard and pde-gh-write to one pde-gh
// binary, which starts its runner as a copy of itself. If the program name
// or first argument picked the wrong mode, gh would let writes through or a
// late approval would never run.
func TestMainPicksModeByName(t *testing.T) {
	cases := []struct {
		name   string
		args   []string
		code   int
		stderr string
	}{
		{name: "write", args: []string{"/home/me/.local/bin/pde-gh-write"}, code: exitRejected,
			stderr: "pde-gh-write: usage: pde-gh-write [--retry] pr {create|edit|comment|review|merge|ready|close|reopen} [gh arguments...]\n"},
		{name: "gh guard", args: []string{"/home/me/.local/bin/gh", "pr", "close", "7"}, code: exitRejected,
			stderr: prWriteMessage + "\n"},
		{name: "runner", args: []string{"pde-gh", approval.RunnerFlag, "ran"}, code: 5,
			stderr: "pde-gh-write: already ran ran, exit 5\npde-gh-write: to ask for approval again, rerun this exact command with --retry before pr\n"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", t.TempDir())
			t.Setenv("PDE_GH_WRITE", "")
			if err := approval.Save(approval.Record{ID: "ran", State: approval.StateRan, Exit: 5}); err != nil {
				t.Fatal(err)
			}
			var output strings.Builder
			approvaltest.Replace[io.Writer](t, &stderr, &output)

			code := Main(tc.args)

			if code != tc.code || output.String() != tc.stderr {
				t.Errorf("exit %d with stderr %q, want %d with %q", code, output.String(), tc.code, tc.stderr)
			}
		})
	}
}
