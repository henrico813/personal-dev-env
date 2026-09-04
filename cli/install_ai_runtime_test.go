package main

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPlannerLaunchersLeaveCodexPackage(t *testing.T) {
	root := t.TempDir()
	cfg := &Config{
		LocalBinDir:       filepath.Join(root, "home", ".local", "bin"),
		OpenCodeConfigDir: filepath.Join(root, "home", ".config", "opencode"),
		CodexConfigDir:    filepath.Join(root, "home", ".codex"),
	}
	local := filepath.Join(cfg.LocalBinDir, "planner")
	openCode := filepath.Join(cfg.OpenCodeConfigDir, "bin", "planner")
	codex := filepath.Join(cfg.CodexConfigDir, "skills", "create-plan", "bin", "planner")
	mustWriteFile(t, local, "old local\n", 0o755)
	mustWriteFile(t, openCode, "old OpenCode\n", 0o755)
	mustWriteFile(t, codex, "transaction owned\n", 0o755)

	if err := backupPlannerLaunchers(cfg, Runner{}); err != nil {
		t.Fatalf("backup Planner launchers: %v", err)
	}
	mustFileContents(t, codex, "transaction owned\n")
	if _, err := os.Stat(local); !os.IsNotExist(err) {
		t.Fatalf("local launcher remains: %v", err)
	}
	if _, err := os.Stat(openCode); !os.IsNotExist(err) {
		t.Fatalf("OpenCode launcher remains: %v", err)
	}

	planner := filepath.Join(root, "runtime", "planner")
	mustWriteFile(t, planner, "planner\n", 0o755)
	if err := installPlannerLaunchers(cfg, planner, Runner{}); err != nil {
		t.Fatalf("install Planner launchers: %v", err)
	}
	mustLinkTarget(t, local, planner)
	mustLinkTarget(t, openCode, planner)
	mustFileContents(t, codex, "transaction owned\n")
}

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

func TestLauncherHelp(t *testing.T) {
	tests := []struct {
		name   string
		binary string
		arg    string
		verify func(*Config, Runner) error
	}{
		{name: "planner", binary: "planner", arg: "help", verify: verifyPlannerLauncher},
		{name: "shim", binary: "opencode-inline-shim", arg: "--help", verify: verifyOpenCodeInlineShimLauncher},
		{name: "vibe", binary: "vibe", arg: "--help", verify: verifyVibeLauncher},
		{name: "surveil", binary: "surveil", arg: "--help", verify: verifySurveilLauncher},
		{name: "pi", binary: "pi", arg: "--help", verify: verifyPiLauncher},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			localBin := filepath.Join(t.TempDir(), ".local", "bin")
			if err := os.MkdirAll(localBin, 0o755); err != nil {
				t.Fatalf("mkdir local bin: %v", err)
			}

			writeStubExecutable(t, filepath.Join(localBin, tt.binary), tt.arg)

			if err := tt.verify(&Config{LocalBinDir: localBin}, Runner{}); err != nil {
				t.Fatalf("verify %s launcher: %v", tt.name, err)
			}
		})
	}
}

func writeRuntimeMarker(t *testing.T, runtimeDir, marker string) {
	t.Helper()
	if err := os.MkdirAll(runtimeDir, 0o755); err != nil {
		t.Fatalf("mkdir runtime: %v", err)
	}
	if err := os.WriteFile(filepath.Join(runtimeDir, "marker"), []byte(marker), 0o644); err != nil {
		t.Fatalf("write runtime marker: %v", err)
	}
}

func readRuntimeMarker(t *testing.T, runtimeDir string) string {
	t.Helper()
	marker, err := os.ReadFile(filepath.Join(runtimeDir, "marker"))
	if err != nil {
		t.Fatalf("read runtime marker: %v", err)
	}
	return string(marker)
}

func TestNodeToolStagesRuntime(t *testing.T) {
	root := t.TempDir()
	cfg := &Config{
		HomeDir:      filepath.Join(root, "home"),
		LocalBinDir:  filepath.Join(root, "home", ".local", "bin"),
		AIRuntimeDir: filepath.Join(root, "home", ".local", "share", "pde", "ai"),
	}
	var output bytes.Buffer

	if err := installNodeTool(cfg, Runner{DryRun: true, Stdout: &output, Stderr: &output}, "pi", "@earendil-works/pi-coding-agent"); err != nil {
		t.Fatalf("install node tool dry run: %v", err)
	}

	stagedRuntimeDir := filepath.Join(cfg.AIRuntimeDir, "pi") + ".tmp"
	dryRun := output.String()
	if !strings.Contains(dryRun, "npm install --prefix "+shellQuote(stagedRuntimeDir)) {
		t.Fatalf("missing staged runtime prefix:\n%s", dryRun)
	}
	if !strings.Contains(dryRun, "DRY-RUN: activate pi runtime") {
		t.Fatalf("missing staged runtime activation:\n%s", dryRun)
	}
}

func TestStagedRuntimeActivation(t *testing.T) {
	runtimeDir := filepath.Join(t.TempDir(), "pi")
	stagedRuntimeDir := runtimeDir + ".tmp"
	writeRuntimeMarker(t, runtimeDir, "live")
	writeRuntimeMarker(t, stagedRuntimeDir, "staged")

	if err := activateStagedRuntime("pi", runtimeDir, stagedRuntimeDir, Runner{}); err != nil {
		t.Fatalf("activate staged runtime: %v", err)
	}
	if got := readRuntimeMarker(t, runtimeDir); got != "staged" {
		t.Fatalf("runtime marker = %q, want %q", got, "staged")
	}
	if _, err := os.Stat(runtimeDir + ".bak"); !os.IsNotExist(err) {
		t.Fatalf("runtime backup should be removed, stat err = %v", err)
	}
}

func TestFailedActivationKeepsRuntime(t *testing.T) {
	runtimeDir := filepath.Join(t.TempDir(), "pi")
	writeRuntimeMarker(t, runtimeDir, "live")

	err := activateStagedRuntime("pi", runtimeDir, runtimeDir+".tmp", Runner{})
	if err == nil {
		t.Fatal("expected activation error")
	}
	if got := readRuntimeMarker(t, runtimeDir); got != "live" {
		t.Fatalf("runtime marker = %q, want %q", got, "live")
	}
	if _, err := os.Stat(runtimeDir + ".bak"); !os.IsNotExist(err) {
		t.Fatalf("runtime backup should be restored, stat err = %v", err)
	}
}

func TestFailedInstallKeepsRuntime(t *testing.T) {
	root := t.TempDir()
	cfg := &Config{
		HomeDir:      filepath.Join(root, "home"),
		LocalBinDir:  filepath.Join(root, "home", ".local", "bin"),
		AIRuntimeDir: filepath.Join(root, "home", ".local", "share", "pde", "ai"),
	}
	runtimeDir := filepath.Join(cfg.AIRuntimeDir, "pi")
	stagedRuntimeDir := runtimeDir + ".tmp"
	writeRuntimeMarker(t, runtimeDir, "live")
	writeRuntimeMarker(t, stagedRuntimeDir, "stale")

	var output bytes.Buffer
	err := installNodeTool(cfg, Runner{Stdout: &output, Stderr: &output}, "pi", "@earendil-works/pi-coding-agent")
	if err == nil {
		t.Fatal("expected install error")
	}
	if got := readRuntimeMarker(t, runtimeDir); got != "live" {
		t.Fatalf("runtime marker = %q, want %q", got, "live")
	}
	if _, err := os.Stat(stagedRuntimeDir); !os.IsNotExist(err) {
		t.Fatalf("staged runtime should be removed, stat err = %v", err)
	}
}
