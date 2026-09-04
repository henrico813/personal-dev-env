package builds

import (
	"os"
	"path/filepath"
	"testing"

	"pde-installer/internal/run"
)

// A blocked binary path is broken rather than missing.
func TestBuildProbeReturnsFilesystemErrors(t *testing.T) {
	t.Parallel()
	home := t.TempDir()
	blockingPath := filepath.Join(home, ".local", "bin")
	if err := os.MkdirAll(filepath.Dir(blockingPath), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blockingPath, []byte("blocked"), 0o644); err != nil {
		t.Fatal(err)
	}
	_, err := New(home, t.TempDir(), t.TempDir(), run.Runner{}).Probe("planner")
	if err == nil {
		t.Fatal("Probe() error = nil")
	}
}
