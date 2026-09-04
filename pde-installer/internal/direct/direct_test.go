package direct

import (
	"os"
	"path/filepath"
	"testing"

	"pde-installer/internal/run"
)

// A blocked font path is broken rather than missing.
func TestFontProbeReturnsFilesystemErrors(t *testing.T) {
	t.Parallel()
	home := t.TempDir()
	blockingPath := filepath.Join(home, ".local", "share", "fonts")
	if err := os.MkdirAll(filepath.Dir(blockingPath), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blockingPath, []byte("blocked"), 0o644); err != nil {
		t.Fatal(err)
	}
	_, err := New(home, t.TempDir(), run.Runner{}).Probe(Fonts()[0])
	if err == nil {
		t.Fatal("Probe() error = nil")
	}
}
