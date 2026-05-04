package main

import (
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"
)

func TestWritePDEPathsEnvPreservesExistingProfileLine(t *testing.T) {
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	pdeConfigDir := filepath.Join(homeDir, ".config", "pde")
	pathsEnv := filepath.Join(pdeConfigDir, "paths.env")

	if err := os.MkdirAll(pdeConfigDir, 0o755); err != nil {
		t.Fatalf("mkdir pde config dir: %v", err)
	}
	if err := os.WriteFile(pathsEnv, []byte("# existing\nexport PDE_PROFILE=\"full\"\n"), 0o644); err != nil {
		t.Fatalf("seed paths.env: %v", err)
	}

	cfg := &Config{RepoRoot: repoRoot, PDEConfigDir: pdeConfigDir}
	if err := writePDEPathsEnv(cfg, Runner{}); err != nil {
		t.Fatalf("write pde paths env: %v", err)
	}

	data, err := os.ReadFile(pathsEnv)
	if err != nil {
		t.Fatalf("read paths.env: %v", err)
	}
	content := string(data)
	if !strings.Contains(content, "export PDE_PROFILE=\"full\"") {
		t.Fatalf("expected existing profile line to be preserved, got:\n%s", content)
	}
	if strings.Contains(content, "PDE_PROFILE=\"minimal\"") {
		t.Fatalf("unexpected generated minimal profile line, got:\n%s", content)
	}
}

func TestLinkConfigBacksUpExistingRegularFileBeforeLinking(t *testing.T) {
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	src := filepath.Join(repoRoot, "source.txt")
	dst := filepath.Join(homeDir, ".config", "pde", "target.txt")

	if err := os.MkdirAll(filepath.Dir(dst), 0o755); err != nil {
		t.Fatalf("mkdir dst parent: %v", err)
	}
	if err := os.WriteFile(src, []byte("managed"), 0o644); err != nil {
		t.Fatalf("write src: %v", err)
	}
	if err := os.WriteFile(dst, []byte("user file"), 0o644); err != nil {
		t.Fatalf("write dst: %v", err)
	}

	if err := linkConfig(src, dst, Runner{}); err != nil {
		t.Fatalf("link config: %v", err)
	}

	info, err := os.Lstat(dst)
	if err != nil {
		t.Fatalf("lstat dst: %v", err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("expected dst to be a symlink")
	}
	if target, err := os.Readlink(dst); err != nil {
		t.Fatalf("readlink dst: %v", err)
	} else if target != src {
		t.Fatalf("unexpected symlink target %q want %q", target, src)
	}

	matches, err := filepath.Glob(dst + ".bak.*")
	if err != nil {
		t.Fatalf("glob backup: %v", err)
	}
	if len(matches) != 1 {
		t.Fatalf("expected one backup, got %d (%v)", len(matches), matches)
	}
	backupData, err := os.ReadFile(matches[0])
	if err != nil {
		t.Fatalf("read backup: %v", err)
	}
	if string(backupData) != "user file" {
		t.Fatalf("unexpected backup contents %q", string(backupData))
	}
}

func TestLinkConfigReplacesWrongSymlinkWithoutBackup(t *testing.T) {
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	src := filepath.Join(repoRoot, "source.txt")
	dst := filepath.Join(homeDir, ".config", "pde", "target.txt")
	wrongSrc := filepath.Join(repoRoot, "wrong.txt")

	if err := os.MkdirAll(filepath.Dir(dst), 0o755); err != nil {
		t.Fatalf("mkdir dst parent: %v", err)
	}
	for _, path := range []string{src, wrongSrc} {
		if err := os.WriteFile(path, []byte(filepath.Base(path)), 0o644); err != nil {
			t.Fatalf("write %s: %v", path, err)
		}
	}
	if err := os.Symlink(wrongSrc, dst); err != nil {
		t.Fatalf("seed wrong symlink: %v", err)
	}

	if err := linkConfig(src, dst, Runner{}); err != nil {
		t.Fatalf("link config: %v", err)
	}

	if target, err := os.Readlink(dst); err != nil {
		t.Fatalf("readlink dst: %v", err)
	} else if target != src {
		t.Fatalf("unexpected symlink target %q want %q", target, src)
	}
	matches, err := filepath.Glob(dst + ".bak.*")
	if err != nil {
		t.Fatalf("glob backup: %v", err)
	}
	if len(matches) != 0 {
		t.Fatalf("expected no backup for wrong symlink, got %v", matches)
	}
}

func TestInstallConfigCreatesManagedSharedLinkSet(t *testing.T) {
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	pdeConfigDir := filepath.Join(homeDir, ".config", "pde")
	pathsEnv := filepath.Join(pdeConfigDir, "paths.env")

	for _, path := range []string{
		filepath.Join(repoRoot, "pde", "config", "zsh", "zshrc"),
		filepath.Join(repoRoot, "pde", "config", "zsh", "zsh_plugins.txt"),
		filepath.Join(repoRoot, "pde", "config", "tmux", "tmux.conf"),
		filepath.Join(repoRoot, "pde", "config", "p10k", "p10k.zsh"),
		filepath.Join(repoRoot, "pde", "config", "bottom", "bottom.toml"),
	} {
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatalf("mkdir source parent: %v", err)
		}
		if err := os.WriteFile(path, []byte(filepath.Base(path)), 0o644); err != nil {
			t.Fatalf("write source %s: %v", path, err)
		}
	}
	if err := os.MkdirAll(pdeConfigDir, 0o755); err != nil {
		t.Fatalf("mkdir pde config dir: %v", err)
	}
	if err := os.WriteFile(pathsEnv, []byte("export PDE_PROFILE=\"shared\"\n"), 0o644); err != nil {
		t.Fatalf("seed paths.env: %v", err)
	}

	cfg := &Config{RepoRoot: repoRoot, HomeDir: homeDir, PDEConfigDir: pdeConfigDir}
	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}

	expectedLinks := map[string]string{
		filepath.Join(homeDir, ".zshrc"):                          filepath.Join(repoRoot, "pde", "config", "zsh", "zshrc"),
		filepath.Join(homeDir, ".zsh_plugins.txt"):                filepath.Join(repoRoot, "pde", "config", "zsh", "zsh_plugins.txt"),
		filepath.Join(homeDir, ".tmux.conf"):                      filepath.Join(repoRoot, "pde", "config", "tmux", "tmux.conf"),
		filepath.Join(homeDir, ".p10k.zsh"):                       filepath.Join(repoRoot, "pde", "config", "p10k", "p10k.zsh"),
		filepath.Join(homeDir, ".config", "bottom", "bottom.toml"): filepath.Join(repoRoot, "pde", "config", "bottom", "bottom.toml"),
	}
	paths := make([]string, 0, len(expectedLinks))
	for dst := range expectedLinks {
		paths = append(paths, dst)
	}
	sort.Strings(paths)
	for _, dst := range paths {
		dst := dst
		t.Run(filepath.Base(dst), func(t *testing.T) {
			target, err := os.Readlink(dst)
			if err != nil {
				t.Fatalf("readlink %s: %v", dst, err)
			}
			if target != expectedLinks[dst] {
				t.Fatalf("unexpected link target %q want %q", target, expectedLinks[dst])
			}
		})
	}

	data, err := os.ReadFile(pathsEnv)
	if err != nil {
		t.Fatalf("read paths.env: %v", err)
	}
	if !strings.Contains(string(data), "export PDE_PROFILE=\"shared\"") {
		t.Fatalf("expected profile line to remain, got:\n%s", string(data))
	}
}
