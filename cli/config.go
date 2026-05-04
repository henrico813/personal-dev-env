package main

import (
	"fmt"
	"os"
	"path/filepath"
)

type Config struct {
	RepoRoot          string
	HomeDir           string
	NvimConfigDir     string
	LocalBinDir       string
	PDEConfigDir      string
	PDERuntimeDir     string
	AIRepoDir         string
	AIRuntimeDir      string
	OpenCodeConfigDir string
	CodexConfigDir    string
	PiAgentDir        string
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
				RepoRoot:          root,
				HomeDir:           homeDir,
				NvimConfigDir:     filepath.Join(homeDir, ".config", "nvim"),
				LocalBinDir:       filepath.Join(homeDir, ".local", "bin"),
				PDEConfigDir:      filepath.Join(homeDir, ".config", "pde"),
				PDERuntimeDir:     filepath.Join(homeDir, ".local", "share", "pde"),
				AIRepoDir:         filepath.Join(root, "ai"),
				AIRuntimeDir:      filepath.Join(homeDir, ".local", "share", "pde", "ai"),
				OpenCodeConfigDir: filepath.Join(homeDir, ".config", "opencode"),
				CodexConfigDir:    filepath.Join(homeDir, ".codex"),
				PiAgentDir:        filepath.Join(homeDir, ".pi", "agent"),
			}, nil
		}
	}

	return nil, fmt.Errorf("repo root not found; pass --repo-root, set PDE_REPO_ROOT, or run from the repo checkout containing pde/config")
}

func findRepoRootFromCwd(cwd string) string {
	for dir := cwd; dir != "" && dir != string(filepath.Separator); dir = filepath.Dir(dir) {
		if _, err := os.Stat(filepath.Join(dir, "pde", "config")); err == nil {
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

	if _, err := os.Stat(filepath.Join(root, "pde", "config")); err != nil {
		return "", false
	}

	return root, true
}

func (c *Config) ObsidianRuntimeDir() string {
	return filepath.Join(c.PDERuntimeDir, "obsidian-headless")
}
