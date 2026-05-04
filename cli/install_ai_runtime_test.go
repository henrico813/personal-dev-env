package main

import (
	"os"
	"path/filepath"
	"testing"
)

func writeStubExecutable(t *testing.T, path, expectedArg string) {
	t.Helper()
	script := "#!/usr/bin/env bash\n" +
		"set -euo pipefail\n" +
		"if [[ \"${1:-}\" != \"" + expectedArg + "\" ]]; then\n" +
		"\techo \"unexpected args: $*\" >&2\n" +
		"\texit 1\n" +
		"fi\n"
	if err := os.WriteFile(path, []byte(script), 0o755); err != nil {
		t.Fatalf("write stub %s: %v", path, err)
	}
}

func TestVerifyPlannerLauncherRunsHelp(t *testing.T) {
	localBin := filepath.Join(t.TempDir(), ".local", "bin")
	if err := os.MkdirAll(localBin, 0o755); err != nil {
		t.Fatalf("mkdir local bin: %v", err)
	}

	plannerPath := filepath.Join(localBin, "planner")
	writeStubExecutable(t, plannerPath, "help")

	cfg := &Config{LocalBinDir: localBin}
	if err := verifyPlannerLauncher(cfg, Runner{}); err != nil {
		t.Fatalf("verify planner launcher: %v", err)
	}
}

func TestVerifyOpenCodeInlineShimLauncherRunsHelp(t *testing.T) {
	localBin := filepath.Join(t.TempDir(), ".local", "bin")
	if err := os.MkdirAll(localBin, 0o755); err != nil {
		t.Fatalf("mkdir local bin: %v", err)
	}

	shimPath := filepath.Join(localBin, "opencode-inline-shim")
	writeStubExecutable(t, shimPath, "--help")

	cfg := &Config{LocalBinDir: localBin}
	if err := verifyOpenCodeInlineShimLauncher(cfg, Runner{}); err != nil {
		t.Fatalf("verify shim launcher: %v", err)
	}
}

func TestVerifyVibeLauncherRunsHelp(t *testing.T) {
	localBin := filepath.Join(t.TempDir(), ".local", "bin")
	if err := os.MkdirAll(localBin, 0o755); err != nil {
		t.Fatalf("mkdir local bin: %v", err)
	}

	vibePath := filepath.Join(localBin, "vibe")
	writeStubExecutable(t, vibePath, "--help")

	cfg := &Config{LocalBinDir: localBin}
	if err := verifyVibeLauncher(cfg, Runner{}); err != nil {
		t.Fatalf("verify vibe launcher: %v", err)
	}
}

func TestVerifyPiLauncherRunsHelp(t *testing.T) {
	localBin := filepath.Join(t.TempDir(), ".local", "bin")
	if err := os.MkdirAll(localBin, 0o755); err != nil {
		t.Fatalf("mkdir local bin: %v", err)
	}

	piPath := filepath.Join(localBin, "pi")
	writeStubExecutable(t, piPath, "--help")

	cfg := &Config{LocalBinDir: localBin}
	if err := verifyPiLauncher(cfg, Runner{}); err != nil {
		t.Fatalf("verify pi launcher: %v", err)
	}
}
