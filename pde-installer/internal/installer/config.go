package installer

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"pde-installer/internal/fsutil"
)

type config struct {
	Home, RepoRoot, LocalBin, PkgSource, PkgPrefix, AquaRoot string
}

func detectConfig(flagRoot string) (config, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return config{}, fmt.Errorf("resolve HOME: %w", err)
	}
	home, err = filepath.Abs(home)
	if err != nil || home == string(filepath.Separator) {
		return config{}, fmt.Errorf("invalid HOME %q", home)
	}
	cwd, err := os.Getwd()
	if err != nil {
		return config{}, err
	}
	root := ""
	for _, candidate := range []string{flagRoot, os.Getenv("PDE_REPO_ROOT"), findRepoRoot(cwd)} {
		if normalized, ok := repository(candidate); ok {
			root = normalized
			break
		}
	}
	if root == "" {
		return config{}, fmt.Errorf("repository root not found; pass --repo-root, set PDE_REPO_ROOT, or run within this checkout")
	}
	cfg := config{
		Home: home, RepoRoot: root, LocalBin: filepath.Join(home, ".local", "bin"),
		PkgSource: filepath.Join(home, ".local", "src", "pkgsrc-2026Q2"), PkgPrefix: filepath.Join(home, ".local", "pkg"),
		AquaRoot: filepath.Join(home, ".local", "share", "aquaproj-aqua"),
	}
	for _, destination := range []string{cfg.LocalBin, cfg.PkgSource, cfg.PkgPrefix, cfg.AquaRoot, filepath.Join(home, ".config"), filepath.Join(home, ".local", "state")} {
		if !within(home, destination) {
			return config{}, fmt.Errorf("destination outside HOME: %s", destination)
		}
		if err := fsutil.GuardHome(home, destination); err != nil {
			return config{}, err
		}
	}
	return cfg, nil
}

func findRepoRoot(start string) string {
	for directory := start; ; directory = filepath.Dir(directory) {
		if _, ok := repository(directory); ok {
			return directory
		}
		if parent := filepath.Dir(directory); parent == directory {
			return ""
		}
	}
}

func repository(candidate string) (string, bool) {
	if candidate == "" {
		return "", false
	}
	root, err := filepath.Abs(candidate)
	if err != nil {
		return "", false
	}
	for _, path := range []string{filepath.Join(root, "chezmoi"), filepath.Join(root, "planner", "go.mod"), filepath.Join(root, "pde-installer", "go.mod")} {
		if _, err := os.Stat(path); err != nil {
			return "", false
		}
	}
	return root, true
}

func within(home, path string) bool {
	relative, err := filepath.Rel(home, filepath.Clean(path))
	return err == nil && relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator))
}
