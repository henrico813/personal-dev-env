package installer

import (
	"bytes"
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
