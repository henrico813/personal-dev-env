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

func TestInstallConfigBacksUpConfigJSONRegularFileAndPreservesProfile(t *testing.T) {
	cfg, configJSON := newInstallConfigFixture(t)
	mustWriteFile(t, configJSON, "{\n  \"profile\": \"shared\"\n}\n", 0o644)

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	content := mustFileContents(t, configJSON, "")
	if !strings.Contains(content, `"profile": "shared"`) {
		t.Fatalf("expected profile field to be preserved, got:\n%s", content)
	}
	if !strings.Contains(content, `"install_path":`) {
		t.Fatalf("expected generated install path, got:\n%s", content)
	}
	backup := mustSingleBackup(t, configJSON)
	mustFileContents(t, backup, "{\n  \"profile\": \"shared\"\n}\n")
}

func TestInstallConfigBacksUpConfigJSONSymlinkToReadableFileAndPreservesProfile(t *testing.T) {
	cfg, configJSON := newInstallConfigFixture(t)
	seed := filepath.Join(cfg.HomeDir, "seed-config.json")
	mustWriteFile(t, seed, "{\n  \"profile\": \"minimal\"\n}\n", 0o644)
	if err := os.Symlink(seed, configJSON); err != nil {
		t.Fatalf("seed symlink config.json: %v", err)
	}

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	content := mustFileContents(t, configJSON, "")
	if !strings.Contains(content, `"profile": "minimal"`) {
		t.Fatalf("expected preserved profile field, got:\n%s", content)
	}
	backup := mustSingleBackup(t, configJSON)
	mustSymlinkTarget(t, backup, seed)
}

func TestInstallConfigPreservesVaultAndOpencodeJSONFields(t *testing.T) {
	cfg, configJSON := newInstallConfigFixture(t)
	mustWriteFile(t, configJSON, "{\n  \"main_vault\": \"/vaults/main\",\n  \"work_vault\": \"/vaults/work\",\n  \"opencode_base_url\": \"http://127.0.0.1:4199\"\n}\n", 0o644)

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	content := mustFileContents(t, configJSON, "")
	if !strings.Contains(content, `"main_vault": "/vaults/main"`) {
		t.Fatalf("expected main vault field to be preserved, got:\n%s", content)
	}
	if !strings.Contains(content, `"work_vault": "/vaults/work"`) {
		t.Fatalf("expected work vault field to be preserved, got:\n%s", content)
	}
	if !strings.Contains(content, `"opencode_base_url": "http://127.0.0.1:4199"`) {
		t.Fatalf("expected opencode base URL to be preserved, got:\n%s", content)
	}
}

func TestInstallConfigRemovesDeprecatedPathsEnv(t *testing.T) {
	cfg, configJSON := newInstallConfigFixture(t)
	mustWriteFile(t, configJSON, "{\n  \"default_vault\": \"main\"\n}\n", 0o644)
	deprecated := filepath.Join(cfg.PDEConfigDir, "paths.env")
	mustWriteFile(t, deprecated, "export PDE_DEFAULT_VAULT=\"main\"\n", 0o644)

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	if _, err := os.Stat(deprecated); !os.IsNotExist(err) {
		t.Fatalf("expected deprecated paths.env to be removed, got err=%v", err)
	}
}

func TestInstallConfigBacksUpConfigJSONSymlinkToDirectoryWithoutProfile(t *testing.T) {
	cfg, configJSON := newInstallConfigFixture(t)
	seedDir := filepath.Join(cfg.HomeDir, "config-json-dir")
	if err := os.MkdirAll(seedDir, 0o755); err != nil {
		t.Fatalf("mkdir seed dir: %v", err)
	}
	if err := os.Symlink(seedDir, configJSON); err != nil {
		t.Fatalf("seed symlink config.json: %v", err)
	}

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	mustFileContents(t, configJSON, "")
	backup := mustSingleBackup(t, configJSON)
	if info, err := os.Lstat(backup); err != nil {
		t.Fatalf("lstat backup: %v", err)
	} else if info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("expected backup to remain symlink")
	}
}

func TestInstallConfigBacksUpConfigJSONDirectoryWithoutProfile(t *testing.T) {
	cfg, configJSON := newInstallConfigFixture(t)
	if err := os.MkdirAll(filepath.Join(configJSON, "nested"), 0o755); err != nil {
		t.Fatalf("mkdir config.json dir: %v", err)
	}
	mustWriteFile(t, filepath.Join(configJSON, "nested", "keep.txt"), "dir contents", 0o644)

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	mustFileContents(t, configJSON, "")
	backup := mustSingleBackup(t, configJSON)
	mustFileContents(t, filepath.Join(backup, "nested", "keep.txt"), "dir contents")
}

func TestExistingPDEPathsEnvLinesReturnsErrorForUnexpectedStatFailure(t *testing.T) {
	path := "bad\x00path"

	_, err := existingPDEPathsEnvLines(path)
	if err == nil {
		t.Fatal("expected error")
	}
	if !strings.Contains(err.Error(), path) {
		t.Fatalf("expected error to include path %q, got %v", path, err)
	}
}

func TestInstallConfigBacksUpBrokenConfigJSONSymlinkWithoutProfile(t *testing.T) {
	cfg, configJSON := newInstallConfigFixture(t)
	brokenTarget := filepath.Join(cfg.HomeDir, "missing-config.json")
	if err := os.Symlink(brokenTarget, configJSON); err != nil {
		t.Fatalf("seed broken symlink: %v", err)
	}

	if err := installConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install config: %v", err)
	}
	mustFileContents(t, configJSON, "")
	backup := mustSingleBackup(t, configJSON)
	mustSymlinkTarget(t, backup, brokenTarget)
}

func TestInstallConfigMissingManagedSourceFailsBeforeMutatingConfigOrLinks(t *testing.T) {
	cfg, configJSON := newInstallConfigFixture(t)
	mustWriteFile(t, configJSON, "{\n  \"profile\": \"shared\"\n}\n", 0o644)
	if err := os.Remove(filepath.Join(cfg.RepoRoot, "pde", "config", "bottom", "bottom.toml")); err != nil {
		t.Fatalf("remove managed source: %v", err)
	}

	if err := installConfig(cfg, Runner{}); err == nil {
		t.Fatal("expected error for missing managed source")
	}
	mustFileContents(t, configJSON, "{\n  \"profile\": \"shared\"\n}\n")
	mustNoBackups(t, configJSON)
	if _, err := os.Lstat(filepath.Join(cfg.HomeDir, ".zshrc")); !os.IsNotExist(err) {
		t.Fatalf("expected home link to remain untouched, got err=%v", err)
	}
}

func newInstallConfigFixture(t *testing.T) (*Config, string) {
	t.Helper()
	repoRoot := t.TempDir()
	homeDir := t.TempDir()
	pdeConfigDir := filepath.Join(homeDir, ".config", "pde")
	if err := os.MkdirAll(pdeConfigDir, 0o755); err != nil {
		t.Fatalf("mkdir pde config dir: %v", err)
	}
	configJSON := filepath.Join(pdeConfigDir, "config.json")
	createManagedSources(t, repoRoot, "")
	return &Config{RepoRoot: repoRoot, HomeDir: homeDir, PDEConfigDir: pdeConfigDir}, configJSON
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
