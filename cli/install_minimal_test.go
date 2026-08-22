package main

import (
	"bytes"
	"errors"
	"path/filepath"
	"strings"
	"testing"
)

func TestInstallTargetHasMinimal(t *testing.T) {
	if _, ok := installTargets()["minimal"]; !ok {
		t.Fatal("expected minimal install target")
	}
}

func TestInstallCmdRunsMinimal(t *testing.T) {
	repoRoot := newMinimalRepoFixture(t)
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	stdout, stderr, err := executeInstallCmd(t, "install", "minimal", "--repo-root", repoRoot, "--dry-run")
	if err != nil {
		t.Fatalf("execute install minimal: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}

	dryRun := stdout.String()
	markers := []string{
		"DRY-RUN: run legacy minimal base",
		"DRY-RUN: write PDE config.json",
		"DRY-RUN: install obsidian.nvim",
		"DRY-RUN: verify cargo",
	}
	for _, marker := range markers {
		if !strings.Contains(dryRun, marker) {
			t.Fatalf("missing %q in dry-run output:\n%s", marker, dryRun)
		}
	}
	if !(strings.Index(dryRun, markers[0]) < strings.Index(dryRun, markers[1]) &&
		strings.Index(dryRun, markers[1]) < strings.Index(dryRun, markers[2]) &&
		strings.Index(dryRun, markers[2]) < strings.Index(dryRun, markers[3])) {
		t.Fatalf("unexpected dry-run order:\n%s", dryRun)
	}
}

func TestLegacyBaseCallsHiddenProfile(t *testing.T) {
	repoRoot := t.TempDir()
	script := "#!/usr/bin/env bash\nset -euo pipefail\n" +
		"test \"$1\" = \"__legacy_minimal_base\"\n"
	mustWriteFile(t, filepath.Join(repoRoot, "pde", "pde"), script, 0o755)

	if err := runLegacyMinimalBase(&Config{RepoRoot: repoRoot}, Runner{}); err != nil {
		t.Fatalf("run legacy base: %v", err)
	}
}

func TestLegacyPathNeedsScript(t *testing.T) {
	err := runLegacyMinimalBase(&Config{RepoRoot: t.TempDir()}, Runner{})
	if err == nil {
		t.Fatal("expected missing legacy script error")
	}
	var installErr *minimalInstallError
	if !errors.As(err, &installErr) {
		t.Fatalf("expected minimalInstallError, got %T", err)
	}
	if installErr.Code != minimalLegacyInstallerMissing {
		t.Fatalf("unexpected error code %v", installErr.Code)
	}
}

func TestLegacyPathNeedsExec(t *testing.T) {
	repoRoot := t.TempDir()
	mustWriteFile(t, filepath.Join(repoRoot, "pde", "pde"), "#!/usr/bin/env bash\nexit 0\n", 0o644)

	err := runLegacyMinimalBase(&Config{RepoRoot: repoRoot}, Runner{})
	if err == nil {
		t.Fatal("expected non-executable legacy script error")
	}
	var installErr *minimalInstallError
	if !errors.As(err, &installErr) {
		t.Fatalf("expected minimalInstallError, got %T", err)
	}
	if installErr.Code != minimalLegacyInstallerNotExecutable {
		t.Fatalf("unexpected error code %v", installErr.Code)
	}
}

func TestInstallObsidianNeedsNvim(t *testing.T) {
	cfg := &Config{NvimConfigDir: filepath.Join(t.TempDir(), ".config", "nvim")}

	err := installObsidian(cfg, Runner{})
	if err == nil {
		t.Fatal("expected missing nvim config error")
	}
	if !strings.Contains(err.Error(), "run pde install minimal first") {
		t.Fatalf("unexpected error %v", err)
	}
}

func TestMinimalDryRunSkipsNvim(t *testing.T) {
	cfg := &Config{
		HomeDir:       t.TempDir(),
		NvimConfigDir: filepath.Join(t.TempDir(), ".config", "nvim"),
		LocalBinDir:   filepath.Join(t.TempDir(), ".local", "bin"),
		PDERuntimeDir: filepath.Join(t.TempDir(), ".local", "share", "pde"),
	}
	var output bytes.Buffer
	runner := Runner{DryRun: true, Stdout: &output, Stderr: &output}

	err := installObsidianWithOptions(cfg, runner, obsidianInstallOptions{skipNvimPreflightOnDryRun: true})
	if err != nil {
		t.Fatalf("install obsidian dry run: %v", err)
	}
	if !strings.Contains(output.String(), "DRY-RUN: install obsidian.nvim") {
		t.Fatalf("missing obsidian dry-run output:\n%s", output.String())
	}
}

func executeInstallCmd(t *testing.T, args ...string) (*bytes.Buffer, *bytes.Buffer, error) {
	t.Helper()
	root := newRootCmd()
	stdout := &bytes.Buffer{}
	stderr := &bytes.Buffer{}
	root.SetOut(stdout)
	root.SetErr(stderr)
	root.SetArgs(args)
	return stdout, stderr, root.Execute()
}

func newMinimalRepoFixture(t *testing.T) string {
	t.Helper()
	repoRoot := t.TempDir()
	createManagedSources(t, repoRoot)
	createChezmoiSource(t, repoRoot)
	mustWriteFile(t, filepath.Join(repoRoot, "pde", "pde"), "#!/usr/bin/env bash\nexit 0\n", 0o755)
	manifest := "[package]\nname = \"surveil\"\nversion = \"0.1.0\"\n"
	mustWriteFile(
		t,
		filepath.Join(repoRoot, "surveil", "Cargo.toml"),
		manifest,
		0o644,
	)
	return repoRoot
}
