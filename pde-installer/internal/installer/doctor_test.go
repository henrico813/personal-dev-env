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

// Archive tools are required even without the full build toolchain.
func TestDoctorRequiresTerminalTools(t *testing.T) {
	t.Setenv("PATH", terminalProbeBin(t, "tar"))
	cfg := terminalTestConfig(t)
	writeInvalidFullMetadata(t, cfg)
	var stderr bytes.Buffer
	err := hostPreflight(cfg, run.Runner{Stdout: &bytes.Buffer{}, Stderr: &stderr}, preflightQuiet)
	if err == nil || !strings.Contains(err.Error(), "doctor found 1 problem(s)") {
		t.Fatalf("hostPreflight() error = %v", err)
	}
	if !strings.Contains(stderr.String(), "tar: install host tar") {
		t.Fatalf("doctor diagnostic = %q", stderr.String())
	}
}

// Terminal installs deliberately omit compilers and full-only metadata.
func TestDoctorSkipsBuildTools(t *testing.T) {
	t.Setenv("PATH", terminalProbeBin(t, "compiler"))
	cfg := terminalTestConfig(t)
	writeInvalidFullMetadata(t, cfg)
	if err := hostPreflight(cfg, run.Runner{Stdout: &bytes.Buffer{}, Stderr: &bytes.Buffer{}}, preflightQuiet); err != nil {
		t.Fatalf("hostPreflight() error = %v", err)
	}
}

func TestDoctorRejectsInvalidProfile(t *testing.T) {
	cfg := terminalTestConfig(t)
	cfg.Profile = profile.Profile("desktop")
	err := hostPreflight(cfg, run.Runner{}, preflightQuiet)
	if err == nil || !strings.Contains(err.Error(), `invalid profile "desktop"`) {
		t.Fatalf("hostPreflight() error = %v", err)
	}
}

func TestCopilotStatusReportsInstalled(t *testing.T) {
	home := t.TempDir()
	xdgConfig := t.TempDir()
	t.Setenv("XDG_CONFIG_HOME", xdgConfig)
	for _, path := range []string{
		filepath.Join(home, ".config", "nvim", "pack", "plugins", "start", "copilot.lua", "lua", "copilot", "init.lua"),
		filepath.Join(xdgConfig, "github-copilot", "auth.db"),
	} {
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, nil, 0o600); err != nil {
			t.Fatal(err)
		}
	}

	var output bytes.Buffer
	if err := reportCopilotStatus(home, &output); err != nil {
		t.Fatalf("reportCopilotStatus() error = %v", err)
	}
	if got := output.String(); got != "status  GitHub Copilot plugin: installed\nstatus  GitHub Copilot credentials: present\n" {
		t.Fatalf("Copilot status = %q", got)
	}
}

func TestCopilotStatusWarnsWhenMissing(t *testing.T) {
	t.Setenv("XDG_CONFIG_HOME", filepath.Join(t.TempDir(), "config"))
	var output bytes.Buffer
	if err := reportCopilotStatus(t.TempDir(), &output); err != nil {
		t.Fatalf("reportCopilotStatus() error = %v", err)
	}
	if got := output.String(); got != "warning GitHub Copilot plugin: missing; run pde-installer install\nwarning GitHub Copilot credentials: missing; run :Copilot auth in Neovim\n" {
		t.Fatalf("Copilot status = %q", got)
	}
}
