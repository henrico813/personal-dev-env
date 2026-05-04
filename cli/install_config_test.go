package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestLinkConfigKeepsCorrectSymlinkInPlace(t *testing.T) {
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	src := filepath.Join(repoRoot, "source.txt")
	dst := filepath.Join(homeDir, ".config", "pde", "target.txt")

	mustWriteFile(t, src, "managed", 0o644)
	if err := os.MkdirAll(filepath.Dir(dst), 0o755); err != nil {
		t.Fatalf("mkdir dst parent: %v", err)
	}
	if err := os.Symlink(src, dst); err != nil {
		t.Fatalf("seed symlink: %v", err)
	}

	if err := linkConfig(src, dst, Runner{}); err != nil {
		t.Fatalf("link config: %v", err)
	}
	mustLinkTarget(t, dst, src)
	mustNoBackups(t, dst)
}

func TestLinkConfigReplacesWrongSymlinkWithoutBackup(t *testing.T) {
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	src := filepath.Join(repoRoot, "source.txt")
	wrongSrc := filepath.Join(repoRoot, "wrong.txt")
	dst := filepath.Join(homeDir, ".config", "pde", "target.txt")

	mustWriteFile(t, src, "managed", 0o644)
	mustWriteFile(t, wrongSrc, "wrong", 0o644)
	if err := os.MkdirAll(filepath.Dir(dst), 0o755); err != nil {
		t.Fatalf("mkdir dst parent: %v", err)
	}
	if err := os.Symlink(wrongSrc, dst); err != nil {
		t.Fatalf("seed wrong symlink: %v", err)
	}

	if err := linkConfig(src, dst, Runner{}); err != nil {
		t.Fatalf("link config: %v", err)
	}
	mustLinkTarget(t, dst, src)
	mustNoBackups(t, dst)
}

func TestLinkConfigRejectsMissingSourceBeforeMutatingDestination(t *testing.T) {
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	src := filepath.Join(repoRoot, "missing.txt")
	dst := filepath.Join(homeDir, ".config", "pde", "target.txt")

	mustWriteFile(t, dst, "user file", 0o644)

	if err := linkConfig(src, dst, Runner{}); err == nil {
		t.Fatal("expected error for missing source")
	}
	mustFileContents(t, dst, "user file")
	mustNoBackups(t, dst)
}

func TestLinkConfigBacksUpExistingFileBeforeLinking(t *testing.T) {
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	src := filepath.Join(repoRoot, "source.txt")
	dst := filepath.Join(homeDir, ".config", "pde", "target.txt")

	mustWriteFile(t, src, "managed", 0o644)
	mustWriteFile(t, dst, "user file", 0o644)

	if err := linkConfig(src, dst, Runner{}); err != nil {
		t.Fatalf("link config: %v", err)
	}
	mustLinkTarget(t, dst, src)
	backup := mustSingleBackup(t, dst)
	mustFileContents(t, backup, "user file")
}

func TestLinkConfigBacksUpExistingDirectoryBeforeLinking(t *testing.T) {
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	src := filepath.Join(repoRoot, "source.txt")
	dst := filepath.Join(homeDir, ".config", "pde", "target.txt")

	mustWriteFile(t, src, "managed", 0o644)
	if err := os.MkdirAll(filepath.Join(dst, "nested"), 0o755); err != nil {
		t.Fatalf("mkdir dst dir: %v", err)
	}
	mustWriteFile(t, filepath.Join(dst, "nested", "keep.txt"), "directory contents", 0o644)

	if err := linkConfig(src, dst, Runner{}); err != nil {
		t.Fatalf("link config: %v", err)
	}
	mustLinkTarget(t, dst, src)
	backup := mustSingleBackup(t, dst)
	mustFileContents(t, filepath.Join(backup, "nested", "keep.txt"), "directory contents")
}

func TestInstallConfigBacksUpPathsEnvRegularFileAndPreservesProfile(t *testing.T) {
	cfg, pathsEnv := newInstallConfigFixture(t)
	mustWriteFile(t, pathsEnv, "# existing\nexport PDE_PROFILE=\"shared\"\n", 0o644)

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	content := mustFileContents(t, pathsEnv, "")
	if !strings.Contains(content, "export PDE_PROFILE=\"shared\"") {
		t.Fatalf("expected profile line to be preserved, got:\n%s", content)
	}
	if !strings.Contains(content, "export PDE_INSTALL_PATH=") {
		t.Fatalf("expected generated install path, got:\n%s", content)
	}
	backup := mustSingleBackup(t, pathsEnv)
	mustFileContents(t, backup, "# existing\nexport PDE_PROFILE=\"shared\"\n")
}

func TestInstallConfigBacksUpPathsEnvSymlinkToReadableFileAndPreservesProfile(t *testing.T) {
	cfg, pathsEnv := newInstallConfigFixture(t)
	seed := filepath.Join(cfg.HomeDir, "seed-paths.env")
	mustWriteFile(t, seed, "export PDE_PROFILE=\"minimal\"\n", 0o644)
	if err := os.Symlink(seed, pathsEnv); err != nil {
		t.Fatalf("seed symlink paths.env: %v", err)
	}

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	content := mustFileContents(t, pathsEnv, "")
	if !strings.Contains(content, "export PDE_PROFILE=\"minimal\"") {
		t.Fatalf("expected preserved profile line, got:\n%s", content)
	}
	backup := mustSingleBackup(t, pathsEnv)
	mustSymlinkTarget(t, backup, seed)
}

func TestInstallConfigBacksUpPathsEnvSymlinkToDirectoryWithoutProfile(t *testing.T) {
	cfg, pathsEnv := newInstallConfigFixture(t)
	seedDir := filepath.Join(cfg.HomeDir, "paths-env-dir")
	if err := os.MkdirAll(seedDir, 0o755); err != nil {
		t.Fatalf("mkdir seed dir: %v", err)
	}
	if err := os.Symlink(seedDir, pathsEnv); err != nil {
		t.Fatalf("seed symlink paths.env: %v", err)
	}

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	content := mustFileContents(t, pathsEnv, "")
	if strings.Contains(content, "PDE_PROFILE") {
		t.Fatalf("did not expect profile preservation for symlink-to-dir paths.env, got:\n%s", content)
	}
	backup := mustSingleBackup(t, pathsEnv)
	if info, err := os.Lstat(backup); err != nil {
		t.Fatalf("lstat backup: %v", err)
	} else if info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("expected backup to remain symlink")
	}
}

func TestInstallConfigBacksUpPathsEnvDirectoryWithoutProfile(t *testing.T) {
	cfg, pathsEnv := newInstallConfigFixture(t)
	if err := os.MkdirAll(filepath.Join(pathsEnv, "nested"), 0o755); err != nil {
		t.Fatalf("mkdir paths.env dir: %v", err)
	}
	mustWriteFile(t, filepath.Join(pathsEnv, "nested", "keep.txt"), "dir contents", 0o644)

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	content := mustFileContents(t, pathsEnv, "")
	if strings.Contains(content, "PDE_PROFILE") {
		t.Fatalf("did not expect profile preservation for directory paths.env, got:\n%s", content)
	}
	backup := mustSingleBackup(t, pathsEnv)
	mustFileContents(t, filepath.Join(backup, "nested", "keep.txt"), "dir contents")
}

func TestInstallConfigBacksUpBrokenPathsEnvSymlinkWithoutProfile(t *testing.T) {
	cfg, pathsEnv := newInstallConfigFixture(t)
	brokenTarget := filepath.Join(cfg.HomeDir, "missing-paths.env")
	if err := os.Symlink(brokenTarget, pathsEnv); err != nil {
		t.Fatalf("seed broken symlink: %v", err)
	}

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	content := mustFileContents(t, pathsEnv, "")
	if strings.Contains(content, "PDE_PROFILE") {
		t.Fatalf("did not expect profile preservation for broken symlink paths.env, got:\n%s", content)
	}
	backup := mustSingleBackup(t, pathsEnv)
	mustSymlinkTarget(t, backup, brokenTarget)
}

func TestInstallConfigMissingManagedSourceFailsBeforeMutatingPathsEnvOrLinks(t *testing.T) {
	cfg, pathsEnv := newInstallConfigFixture(t)
	mustWriteFile(t, pathsEnv, "export PDE_PROFILE=\"shared\"\n", 0o644)
	createManagedSources(t, cfg.RepoRoot, filepath.Join("pde", "config", "bottom", "bottom.toml"))

	if err := installConfig(cfg, Runner{}); err == nil {
		t.Fatal("expected error for missing managed source")
	}
	mustFileContents(t, pathsEnv, "export PDE_PROFILE=\"shared\"\n")
	mustNoBackups(t, pathsEnv)
	if _, err := os.Lstat(filepath.Join(cfg.HomeDir, ".zshrc")); !os.IsNotExist(err) {
		t.Fatalf("expected home link to remain untouched, got err=%v", err)
	}
}

func newInstallConfigFixture(t *testing.T) (*Config, string) {
	t.Helper()
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	pdeConfigDir := filepath.Join(homeDir, ".config", "pde")
	pathsEnv := filepath.Join(pdeConfigDir, "paths.env")
	createManagedSources(t, repoRoot, "")
	return &Config{RepoRoot: repoRoot, HomeDir: homeDir, PDEConfigDir: pdeConfigDir}, pathsEnv
}

func createManagedSources(t *testing.T, repoRoot, missingRel string) {
	t.Helper()
	for _, rel := range []string{
		filepath.Join("pde", "config", "zsh", "zshrc"),
		filepath.Join("pde", "config", "zsh", "zsh_plugins.txt"),
		filepath.Join("pde", "config", "tmux", "tmux.conf"),
		filepath.Join("pde", "config", "p10k", "p10k.zsh"),
		filepath.Join("pde", "config", "bottom", "bottom.toml"),
	} {
		if rel == missingRel {
			continue
		}
		path := filepath.Join(repoRoot, rel)
		mustWriteFile(t, path, filepath.Base(path), 0o644)
	}
}

func mustWriteFile(t *testing.T, path, content string, perm os.FileMode) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("mkdir parent for %s: %v", path, err)
	}
	if err := os.WriteFile(path, []byte(content), perm); err != nil {
		t.Fatalf("write %s: %v", path, err)
	}
}

func mustLinkTarget(t *testing.T, path, want string) {
	t.Helper()
	if target, err := os.Readlink(path); err != nil {
		t.Fatalf("readlink %s: %v", path, err)
	} else if target != want {
		t.Fatalf("unexpected symlink target %q want %q", target, want)
	}
}

func mustSymlinkTarget(t *testing.T, path, want string) {
	t.Helper()
	if info, err := os.Lstat(path); err != nil {
		t.Fatalf("lstat %s: %v", path, err)
	} else if info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("expected symlink at %s", path)
	}
	mustLinkTarget(t, path, want)
}

func mustNoBackups(t *testing.T, path string) {
	t.Helper()
	matches, err := filepath.Glob(path + ".bak.*")
	if err != nil {
		t.Fatalf("glob backups for %s: %v", path, err)
	}
	if len(matches) != 0 {
		t.Fatalf("expected no backups for %s, got %v", path, matches)
	}
}

func mustSingleBackup(t *testing.T, path string) string {
	t.Helper()
	matches, err := filepath.Glob(path + ".bak.*")
	if err != nil {
		t.Fatalf("glob backups for %s: %v", path, err)
	}
	if len(matches) != 1 {
		t.Fatalf("expected one backup for %s, got %v", path, matches)
	}
	return matches[0]
}

func mustFileContents(t *testing.T, path, want string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	content := string(data)
	if want != "" && content != want {
		t.Fatalf("unexpected contents for %s:\nwant %q\n got %q", path, want, content)
	}
	return content
}
