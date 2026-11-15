package chezmoi

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

// A shim that execs the guard keeps the pid, as aqua does when its config has no gh.
// Without the stop, gh spins on one CPU forever and blocks the timed update.
func TestGuardStopsLoopThroughShim(t *testing.T) {
	home, path := installGHGuard(t, `exec "$GUARD" "$@"`)

	output, err := runGHGuard(t, home, path, "pr", "list")

	if err == nil {
		t.Fatalf("guard exited 0 after looping through the shim: %s", output)
	}
	if !strings.Contains(output, "gh guard found itself again") {
		t.Fatalf("output = %q, want the loop message", output)
	}
}

// Timed units start gh with no AQUA_GLOBAL_CONFIG, and the shim needs it to find gh.
// Without the default, every gh call from a unit loops or fails.
func TestGuardDefaultsAquaConfig(t *testing.T) {
	home, path := installGHGuard(t, `printf '%s\n' "$AQUA_GLOBAL_CONFIG"`)

	output, err := runGHGuard(t, home, path, "pr", "list")

	if err != nil {
		t.Fatalf("%v: %s", err, output)
	}
	if want := filepath.Join(home, ".config/aquaproj-aqua/aqua.yaml") + "\n"; output != want {
		t.Fatalf("output = %q, want %q", output, want)
	}
}

// Renders the gh guard into home's bin and puts a shim gh after it on PATH.
// Returns home and that PATH.
func installGHGuard(t *testing.T, shimBody string) (string, string) {
	t.Helper()
	home := t.TempDir()
	guardDir := filepath.Join(home, "bin")
	shimDir := filepath.Join(home, "shim")
	for _, dir := range []string{guardDir, shimDir} {
		if err := os.MkdirAll(dir, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	guard := renderProfileTemplate(t, "dot_local/bin/executable_gh.tmpl", "full")
	if err := os.WriteFile(filepath.Join(guardDir, "gh"), []byte(guard), 0o755); err != nil {
		t.Fatal(err)
	}
	shim := "#!/usr/bin/env bash\n" + shimBody + "\n"
	if err := os.WriteFile(filepath.Join(shimDir, "gh"), []byte(shim), 0o755); err != nil {
		t.Fatal(err)
	}
	return home, guardDir + ":" + shimDir + ":/usr/bin:/bin"
}

// Runs the gh guard with a clean environment and a timeout, so a loop fails the test.
func runGHGuard(t *testing.T, home, path string, args ...string) (string, error) {
	t.Helper()
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()
	command := exec.CommandContext(ctx, filepath.Join(home, "bin", "gh"), args...)
	command.Env = []string{"HOME=" + home, "PATH=" + path, "GUARD=" + filepath.Join(home, "bin", "gh")}
	output, err := command.CombinedOutput()
	return string(output), err
}
