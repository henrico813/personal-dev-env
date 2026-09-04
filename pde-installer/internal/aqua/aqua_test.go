package aqua

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/run"
)

// A broken executable needs repair instead of another installation.
func TestAquaProbeReturnsCommandErrors(t *testing.T) {
	t.Parallel()
	home := t.TempDir()
	binary := filepath.Join(home, ".local", "share", "aquaproj-aqua", "bin", "aqua")
	if err := os.MkdirAll(filepath.Dir(binary), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(binary, []byte("#!/bin/sh\nexit 9\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	_, status, err := New(home, t.TempDir(), run.Runner{}).Probe()
	if err == nil || status != "" || !strings.Contains(err.Error(), "exit status 9") {
		t.Fatalf("Probe() = _, %q, %v", status, err)
	}
	if _, status := New(home, t.TempDir(), run.Runner{}).Status(); status != "error" {
		t.Fatalf("Status() state = %q", status)
	}
}

// Missing Aqua tools must not be reported as current.
func TestAquaProbeReportsMissingTool(t *testing.T) {
	t.Parallel()
	_, status, err := New(t.TempDir(), t.TempDir(), run.Runner{}).ToolProbe("fd", "v8.3.1")
	if err != nil || status != "missing" {
		t.Fatalf("ToolProbe() = _, %q, %v", status, err)
	}
}
