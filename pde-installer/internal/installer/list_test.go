package installer

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/profile"
	"pde-installer/internal/run"
)

// Terminal listing must not parse metadata for unselected backends.
func TestListSkipsFullMetadata(t *testing.T) {
	t.Setenv("PATH", terminalProbeBin(t, ""))
	cfg := terminalTestConfig(t)
	writeInvalidFullMetadata(t, cfg)
	var output bytes.Buffer
	if err := list(cfg, run.Runner{Stdout: &output, Stderr: &bytes.Buffer{}}); err != nil {
		t.Fatalf("list() error = %v", err)
	}
	if !strings.Contains(output.String(), "aqua\tya\tv25.5.31\t\tmissing\n") {
		t.Fatalf("list output omits exact ya row: %s", output.String())
	}
	if strings.Contains(output.String(), "@opencode/cli") || strings.Contains(output.String(), "opencode-inline-shim") || strings.Contains(output.String(), "@anthropic-ai/claude-code") || strings.Contains(output.String(), "neovim") {
		t.Fatalf("list output includes full-only item: %s", output.String())
	}
}

// Terminal builds no tools of its own, but a desktop still builds its player
// into HOME.
func TestListShowsTerminalDesktopsWallpaperPlayer(t *testing.T) {
	t.Setenv("PATH", terminalProbeBin(t, ""))
	cfg := terminalTestConfig(t)
	cfg.Desktop = "i3"
	if err := os.MkdirAll(cfg.LocalBin, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(cfg.LocalBin, "xwinwrap"), []byte("#!/bin/sh\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	var output bytes.Buffer

	if err := list(cfg, run.Runner{Stdout: &output, Stderr: &bytes.Buffer{}}); err != nil {
		t.Fatalf("list() error = %v", err)
	}

	if !strings.Contains(output.String(), "local\txwinwrap\t\t\tinstalled\n") {
		t.Fatalf("list output omits installed xwinwrap: %s", output.String())
	}
}

func TestListRejectsInvalidProfile(t *testing.T) {
	cfg := terminalTestConfig(t)
	cfg.Profile = profile.Profile("desktop")
	err := list(cfg, run.Runner{})
	if err == nil || !strings.Contains(err.Error(), `invalid profile "desktop"`) {
		t.Fatalf("list() error = %v", err)
	}
}
