package installer

import (
	"bytes"
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
	if strings.Contains(output.String(), "opencode-ai") || strings.Contains(output.String(), "neovim") {
		t.Fatalf("list output includes full-only item: %s", output.String())
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
