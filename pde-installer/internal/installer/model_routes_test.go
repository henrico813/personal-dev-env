package installer

import (
	"bytes"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"pde-installer/internal/run"
	"pde-installer/internal/testfile"
)

// The check runs after the install committed its files, so any failure must
// warn and return nil. An error here would turn a missing model into a failed
// install.
func TestFailedRouteCheckWarnsKeepsInstall(t *testing.T) {
	tests := []struct {
		name   string
		script string
		want   string
	}{
		{
			name:   "exit-1",
			script: "print('kind code-review has no model')\nraise SystemExit(1)\n",
			want:   "kind code-review has no model",
		},
		{
			name:   "exit-2",
			script: "import sys\nprint('cannot read opencode models', file=sys.stderr)\nraise SystemExit(2)\n",
			want:   "cannot read opencode models",
		},
		{
			name:   "hang",
			script: "import subprocess\nsubprocess.run(['sleep', '60'])\n",
			want:   "timed out after 200ms",
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			home := newHomeWithRouteCheck(t, tt.script)
			var stderr bytes.Buffer
			runner := run.Runner{Stdout: &bytes.Buffer{}, Stderr: &stderr}

			err := checkModelRoutes(home, runner, 200*time.Millisecond)

			if err != nil {
				t.Fatalf("route check failed the install: %v", err)
			}
			for _, want := range []string{"warning: pde_route.py check", tt.want} {
				if !strings.Contains(stderr.String(), want) {
					t.Errorf("stderr = %q, want substring %q", stderr.String(), want)
				}
			}
		})
	}
}

// Writes script as the installed pde_route.py in a new home.
func newHomeWithRouteCheck(t *testing.T, script string) string {
	t.Helper()
	home := t.TempDir()
	t.Setenv("PATH", "/usr/bin:/bin")
	path := filepath.Join(home, ".agents", "skills", "model-routing", "scripts", "pde_route.py")
	testfile.WriteExecutable(t, path, script)
	return home
}
