package internal

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// Skills name branches, Vibe keys and run folders from this output, so a wrong
// key names the work after the wrong plan.
func TestKeyPrintsLowercaseKeyOrNothing(t *testing.T) {
	dir := t.TempDir()
	for _, name := range []string{
		"ABC-12 Approvals file.md",
		"XYZ-07B First refactor.md",
		"ABC-14.md",
		"DESIGN-Approvals file.md",
	} {
		if err := os.WriteFile(
			filepath.Join(
				dir,
				name,
			),
			nil,
			0o644,
		); err != nil {
			t.Fatal(err)
		}
	}

	tests := []struct {
		name     string
		plan     string
		wantExit int
		wantOut  string
		wantErr  string
	}{
		{
			name:    "key before the title",
			plan:    "ABC-12 Approvals file.md",
			wantOut: "abc-12\n",
		},
		{
			name:    "letters after the digits",
			plan:    "XYZ-07B First refactor.md",
			wantOut: "xyz-07b\n",
		},
		{
			name:    "key with no title",
			plan:    "ABC-14.md",
			wantOut: "abc-14\n",
		},
		{
			name:     "design doc has no key",
			plan:     "DESIGN-Approvals file.md",
			wantExit: 1,
			wantErr:  "does not start with an issue key",
		},
		{
			name:     "missing plan file",
			plan:     "ABC-13 Missing file.md",
			wantExit: 1,
			wantErr:  "no such file",
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			var stdout bytes.Buffer
			var stderr bytes.Buffer

			exit := Execute(
				[]string{
					"key",
					filepath.Join(
						dir,
						tc.plan,
					),
				},
				&stdout,
				&stderr,
			)

			if exit != tc.wantExit || stdout.String() != tc.wantOut || !strings.Contains(stderr.String(), tc.wantErr) {
				t.Errorf(
					"exit %d, stdout %q, stderr %q; want exit %d, stdout %q, stderr containing %q",
					exit,
					stdout.String(),
					stderr.String(),
					tc.wantExit,
					tc.wantOut,
					tc.wantErr,
				)
			}
		})
	}
}
