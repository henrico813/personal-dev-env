package main

import (
	"os"
	"path/filepath"
	"testing"
)

func TestDetectConfigFindsRepoRootWithConfigDir(t *testing.T) {
	repoRoot := t.TempDir()
	if err := os.MkdirAll(filepath.Join(repoRoot, "pde", "config"), 0o755); err != nil {
		t.Fatalf("mkdir config dir: %v", err)
	}
	cwd := filepath.Join(repoRoot, "nested", "dir")
	if err := os.MkdirAll(cwd, 0o755); err != nil {
		t.Fatalf("mkdir cwd: %v", err)
	}
	oldWD, err := os.Getwd()
	if err != nil {
		t.Fatalf("getwd: %v", err)
	}
	t.Cleanup(func() { _ = os.Chdir(oldWD) })
	if err := os.Chdir(cwd); err != nil {
		t.Fatalf("chdir: %v", err)
	}

	cfg, err := detectConfig("")
	if err != nil {
		t.Fatalf("detect config: %v", err)
	}
	if cfg.RepoRoot != repoRoot {
		t.Fatalf("unexpected repo root %q want %q", cfg.RepoRoot, repoRoot)
	}
}

func TestDetectConfigRejectsMissingConfigDir(t *testing.T) {
	repoRoot := t.TempDir()

	if _, ok := normalizeRepoRoot(repoRoot); ok {
		t.Fatal("expected repo root without pde/config to be rejected")
	}
}

func TestDetectConfigRejectsConfigFile(t *testing.T) {
	repoRoot := t.TempDir()
	configPath := filepath.Join(repoRoot, "pde", "config")
	if err := os.MkdirAll(filepath.Dir(configPath), 0o755); err != nil {
		t.Fatalf("mkdir config parent: %v", err)
	}
	if err := os.WriteFile(configPath, []byte("not a dir"), 0o644); err != nil {
		t.Fatalf("write config file: %v", err)
	}

	if _, ok := normalizeRepoRoot(repoRoot); ok {
		t.Fatal("expected repo root with file pde/config to be rejected")
	}
}
