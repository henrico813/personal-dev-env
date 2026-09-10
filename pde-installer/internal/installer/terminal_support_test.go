package installer

import (
	"os"
	"path/filepath"
	"testing"

	"pde-installer/internal/profile"
)

func writeInvalidFullMetadata(t *testing.T, cfg config) {
	t.Helper()
	for _, directory := range []string{
		filepath.Join(cfg.Home, ".local", "share", "pde", "npm"),
		filepath.Join(cfg.Home, ".local", "share", "pde", "releases"),
		filepath.Join(cfg.RepoRoot, "chezmoi", "dot_config", "opencode"),
		filepath.Join(cfg.RepoRoot, "pde-installer"),
	} {
		if err := os.MkdirAll(directory, 0o755); err != nil {
			t.Fatal(err)
		}
	}
	if err := os.WriteFile(filepath.Join(cfg.Home, ".local", "share", "pde", "npm", "package.json"), []byte("invalid"), 0o644); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(cfg.Home, ".local", "share", "pde", "releases", ".pde-state.json"), []byte("invalid"), 0o644); err != nil {
		t.Fatal(err)
	}
	for _, path := range []string{
		filepath.Join(cfg.RepoRoot, "chezmoi", ".chezmoiexternal.toml.tmpl"),
		filepath.Join(cfg.RepoRoot, "chezmoi", ".chezmoiignore.tmpl"),
		filepath.Join(cfg.RepoRoot, "chezmoi", "dot_zshrc.tmpl"),
		filepath.Join(cfg.RepoRoot, "chezmoi", "dot_tmux.conf.tmpl"),
	} {
		if err := os.WriteFile(path, nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	for _, path := range []string{
		filepath.Join(cfg.RepoRoot, "chezmoi", "dot_config", "opencode", "modify_opencode.json"),
		filepath.Join(cfg.RepoRoot, "chezmoi", "dot_config", "opencode", "modify_opencode-mem.jsonc"),
		filepath.Join(cfg.RepoRoot, "pde-installer", "package.json"),
		filepath.Join(cfg.RepoRoot, "pde-installer", "package-lock.json"),
	} {
		if err := os.WriteFile(path, []byte("invalid"), 0o644); err != nil {
			t.Fatal(err)
		}
	}
}

func terminalTestConfig(t *testing.T) config {
	t.Helper()
	home := t.TempDir()
	repoRoot := t.TempDir()
	for _, path := range []string{
		filepath.Join(repoRoot, "chezmoi", ".chezmoiexternal.toml.tmpl"),
		filepath.Join(repoRoot, "chezmoi", ".chezmoiignore.tmpl"),
		filepath.Join(repoRoot, "chezmoi", "dot_zshrc.tmpl"),
		filepath.Join(repoRoot, "chezmoi", "dot_tmux.conf.tmpl"),
		filepath.Join(repoRoot, "chezmoi", "dot_config", "aquaproj-aqua", "aqua.yaml"),
		filepath.Join(repoRoot, "chezmoi", "dot_config", "aquaproj-aqua", "aqua-checksums.json"),
		filepath.Join(repoRoot, "chezmoi", "dot_config", "aquaproj-aqua", "aqua-terminal.yaml"),
		filepath.Join(repoRoot, "chezmoi", "dot_config", "aquaproj-aqua", "aqua-terminal-checksums.json"),
	} {
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	return config{Home: home, RepoRoot: repoRoot, LocalBin: filepath.Join(home, ".local", "bin"), AquaRoot: filepath.Join(home, ".local", "share", "aquaproj-aqua"), Profile: profile.Terminal}
}

func terminalProbeBin(t *testing.T, missing string) string {
	t.Helper()
	bin := t.TempDir()
	for _, name := range []string{"apt-get", "dpkg-query", "sudo", "sh", "tar", "gzip", "xz", "unzip", "sed", "awk", "grep", "file", "curl", "cc", "gcc", "clang", "c++", "g++", "clang++"} {
		if name == missing || missing == "compiler" && (name == "cc" || name == "gcc" || name == "clang" || name == "c++" || name == "g++" || name == "clang++") {
			continue
		}
		contents := "#!/bin/sh\n"
		if name == "dpkg-query" {
			contents += "printf 'ii\\t1\\n'\n"
		} else {
			contents += "exit 0\n"
		}
		if err := os.WriteFile(filepath.Join(bin, name), []byte(contents), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	return bin
}
