package main

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestInstallAIToolsDryRunChecksCargoBeforeMutations(t *testing.T) {
	root := t.TempDir()
	cfg := &Config{
		RepoRoot:          root,
		HomeDir:           filepath.Join(root, "home"),
		LocalBinDir:       filepath.Join(root, "home", ".local", "bin"),
		AIRepoDir:         filepath.Join(root, "ai"),
		AIRuntimeDir:      filepath.Join(root, "home", ".local", "share", "pde", "ai"),
		OpenCodeConfigDir: filepath.Join(root, "home", ".config", "opencode"),
		CodexConfigDir:    filepath.Join(root, "home", ".codex"),
		PiAgentDir:        filepath.Join(root, "home", ".pi", "agent"),
	}

	if err := os.MkdirAll(filepath.Join(cfg.OpenCodeConfigDir, "agents"), 0o755); err != nil {
		t.Fatalf("mkdir managed agents dir: %v", err)
	}

	var output bytes.Buffer
	runner := Runner{DryRun: true, Stdout: &output, Stderr: &output}

	if err := installAITools(cfg, runner); err != nil {
		t.Fatalf("install ai tools dry run: %v", err)
	}

	dryRun := output.String()
	cargo := strings.Index(dryRun, "DRY-RUN: verify cargo")
	backup := strings.Index(dryRun, "DRY-RUN: backup existing config")
	plannerBuild := strings.Index(dryRun, "DRY-RUN: build planner")
	shimBuild := strings.Index(dryRun, "DRY-RUN: build opencode inline shim")
	vibe := strings.Index(dryRun, "DRY-RUN: install vibe")
	node := strings.Index(dryRun, "DRY-RUN: install Node 22")

	if cargo == -1 || backup == -1 || plannerBuild == -1 || shimBuild == -1 || vibe == -1 || node == -1 {
		t.Fatalf("missing expected dry-run output:\n%s", dryRun)
	}
	if cargo > backup || cargo > plannerBuild || cargo > shimBuild {
		t.Fatalf("cargo preflight should run before mutable work:\n%s", dryRun)
	}
	if vibe > node {
		t.Fatalf("vibe install should run before Node setup:\n%s", dryRun)
	}
}
