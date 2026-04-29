package main

import (
	"fmt"
	"os"
	"path/filepath"
)

type Config struct {
	RepoRoot      string
	HomeDir       string
	NvimConfigDir string
	RuntimeDir    string
	LocalBinDir   string
}

func detectConfig(flagRepoRoot string) (*Config, error) {
	homeDir, err := os.UserHomeDir()
	if err != nil {
		return nil, fmt.Errorf("resolve home directory: %w", err)
	}

	cwd, err := os.Getwd()
	if err != nil {
		return nil, fmt.Errorf("resolve cwd: %w", err)
	}

	for _, candidate := range []string{
		flagRepoRoot,
		os.Getenv("PDE_REPO_ROOT"),
		findRepoRootFromCwd(cwd),
	} {
		if root, ok := normalizeRepoRoot(candidate); ok {
			return &Config{
				RepoRoot:      root,
				HomeDir:       homeDir,
				NvimConfigDir: filepath.Join(homeDir, ".config", "nvim"),
				RuntimeDir:    filepath.Join(homeDir, ".local", "share", "pde", "obsidian-headless"),
				LocalBinDir:   filepath.Join(homeDir, ".local", "bin"),
			}, nil
		}
	}

	return nil, fmt.Errorf("repo root not found; pass --repo-root, set PDE_REPO_ROOT, or run from the repo checkout")
}

func findRepoRootFromCwd(cwd string) string {
	for dir := cwd; dir != "" && dir != string(filepath.Separator); dir = filepath.Dir(dir) {
		if _, err := os.Stat(filepath.Join(dir, "pde", "pde")); err == nil {
			return dir
		}
	}
	return ""
}

func normalizeRepoRoot(candidate string) (string, bool) {
	if candidate == "" {
		return "", false
	}

	root, err := filepath.Abs(candidate)
	if err != nil {
		return "", false
	}

	if _, err := os.Stat(filepath.Join(root, "pde", "pde")); err != nil {
		return "", false
	}
	if _, err := os.Stat(filepath.Join(root, "pde", "config", "nvim", "init.lua")); err != nil {
		return "", false
	}

	return root, true
}
