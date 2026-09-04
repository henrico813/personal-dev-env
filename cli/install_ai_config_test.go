package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestAIConfigPreservesSkillBoundaries(t *testing.T) {
	cfg := newAIConfigFixture(t)
	agents := filepath.Join(cfg.HomeDir, ".agents", "skills", "personal", "SKILL.md")
	codex := filepath.Join(cfg.CodexConfigDir, "skills", "personal", "SKILL.md")
	mustWriteFile(t, agents, "personal agents\n", 0o644)
	mustWriteFile(t, codex, "personal codex\n", 0o644)
	legacy := filepath.Join(cfg.HomeDir, ".agents", "skills", "git-messages.backup.20260903_120000")
	mustWriteFile(t, filepath.Join(legacy, "SKILL.md"), "---\nname: git-messages\ndescription: legacy\n---\n", 0o644)
	planner := filepath.Join(cfg.AIRuntimeDir, "planner", "planner")
	mustWriteFile(t, planner, "planner\n", 0o755)

	if err := syncAIConfig(cfg, Runner{}); err != nil { t.Fatalf("first sync: %v", err) }
	mustFileContents(t, filepath.Join(cfg.HomeDir, ".agents", "skills", "self-improvement", "references", "placement.md"), "placement\n")
	mustFileContents(t, agents, "personal agents\n")
	mustFileContents(t, codex, "personal codex\n")
	mustLinkTarget(t, filepath.Join(cfg.CodexConfigDir, "skills", "create-plan", "bin", "planner"), planner)
	if _, err := os.Stat(legacy); !os.IsNotExist(err) { t.Fatalf("legacy backup remains: %v", err) }
	migrated, _ := filepath.Glob(filepath.Join(cfg.AIRuntimeDir, "ai-config-backups", "*", "legacy-agents-skills", filepath.Base(legacy), "SKILL.md"))
	if len(migrated) != 1 { t.Fatalf("migrated backups = %v", migrated) }

	if err := os.RemoveAll(filepath.Join(cfg.AIRepoDir, "skills", "self-improvement")); err != nil { t.Fatal(err) }
	if err := syncAIConfig(cfg, Runner{}); err != nil { t.Fatalf("second sync: %v", err) }
	for _, root := range []string{filepath.Join(cfg.HomeDir, ".agents", "skills"), filepath.Join(cfg.CodexConfigDir, "skills")} {
		if _, err := os.Stat(filepath.Join(root, "self-improvement")); !os.IsNotExist(err) { t.Fatalf("stale skill remains: %v", err) }
	}
	mustFileContents(t, agents, "personal agents\n")
	mustFileContents(t, codex, "personal codex\n")
	mustLinkTarget(t, filepath.Join(cfg.CodexConfigDir, "skills", "create-plan", "bin", "planner"), planner)
}

func TestAIConfigRejectsSkillCollisions(t *testing.T) {
	for _, root := range []func(*Config) string{
		func(c *Config) string { return filepath.Join(c.HomeDir, ".agents", "skills", "self-improvement", "SKILL.md") },
		func(c *Config) string { return filepath.Join(c.CodexConfigDir, "skills", "self-improvement", "SKILL.md") },
	} {
		cfg := newAIConfigFixture(t)
		mustWriteFile(t, filepath.Join(cfg.OpenCodeConfigDir, "AGENTS.md"), "old config\n", 0o644)
		mustWriteFile(t, root(cfg), "user skill\n", 0o644)
		err := syncAIConfig(cfg, Runner{})
		if err == nil || !strings.Contains(err.Error(), "unmanaged skill collision") { t.Fatalf("collision error = %v", err) }
		mustFileContents(t, root(cfg), "user skill\n")
		if _, err := os.Stat(filepath.Join(cfg.AIRuntimeDir, "ai-config-backups")); !os.IsNotExist(err) { t.Fatalf("preflight created backups: %v", err) }
	}
}

func TestAIConfigAdoptsMatchingLegacySkills(t *testing.T) {
	cfg := newAIConfigFixture(t)
	source := filepath.Join(cfg.AIRepoDir, "skills", "git-messages")
	for _, dst := range []string{filepath.Join(cfg.HomeDir, ".agents", "skills", "git-messages"), filepath.Join(cfg.CodexConfigDir, "skills", "git-messages")} {
		if err := copyTree(source, dst); err != nil { t.Fatal(err) }
	}
	source = filepath.Join(cfg.AIRepoDir, "codex", "skills", "create-plan")
	dst := filepath.Join(cfg.CodexConfigDir, "skills", "create-plan")
	if err := copyTree(source, dst); err != nil { t.Fatal(err) }
	mustWriteFile(t, filepath.Join(dst, "bin", "planner"), "legacy launcher\n", 0o755)
	if err := syncAIConfig(cfg, Runner{}); err != nil { t.Fatalf("adopt skills: %v", err) }
	data := mustFileContents(t, skillOwnershipPath(cfg), "")
	for _, name := range []string{"git-messages", "self-improvement", "create-plan"} {
		if !strings.Contains(data, `"`+name+`"`) { t.Fatalf("ownership missing %s", name) }
	}
}

func TestAIConfigValidatesSkillMetadata(t *testing.T) {
	contents := []string{"# Skill\n", "---\ndescription: broken\n---\n", "---\nname: another-skill\n---\n", "---\nname: git-messages\n", "---\nname: git-messages\n---\n"}
	for i, content := range contents {
		cfg := newAIConfigFixture(t)
		path := filepath.Join(cfg.AIRepoDir, "skills", "git-messages", "SKILL.md")
		if i == len(contents)-1 {
			target := filepath.Join(t.TempDir(), "SKILL.md"); mustWriteFile(t, target, content, 0o644); os.Remove(path); os.Symlink(target, path)
		} else { mustWriteFile(t, path, content, 0o644) }
		if err := syncAIConfig(cfg, Runner{}); err == nil { t.Fatalf("case %d: expected metadata error", i) }
	}
}

func TestAIConfigRejectsOwnershipState(t *testing.T) {
	for _, data := range []string{"{", `{"version":2}`, `{"version":1,"shared":["git-messages","git-messages"]}`, `{"version":1,"codex":["BadName"]}`} {
		cfg := newAIConfigFixture(t)
		mustWriteFile(t, skillOwnershipPath(cfg), data, 0o644)
		if err := syncAIConfig(cfg, Runner{}); err == nil { t.Fatalf("expected ownership error") }
		if _, err := os.Stat(filepath.Join(cfg.AIRuntimeDir, "ai-config-backups")); !os.IsNotExist(err) { t.Fatalf("created backups: %v", err) }
	}
}

func TestAIConfigPreflightsEverySource(t *testing.T) {
	paths := []func(*Config) string{
		func(c *Config) string { return filepath.Join(c.AIRepoDir, "opencode", "agents") },
		func(c *Config) string { return filepath.Join(c.AIRepoDir, "opencode", "commands") },
		func(c *Config) string { return filepath.Join(c.AIRepoDir, "AGENTS.md") },
		func(c *Config) string { return filepath.Join(c.AIRepoDir, "codex", "skills") },
		func(c *Config) string { return filepath.Join(c.AIRepoDir, "skills") },
		func(c *Config) string { return filepath.Join(c.AIRepoDir, "pi", "agent", "settings.json") },
	}
	for i, path := range paths {
		for _, dryRun := range []bool{false, true} {
			cfg := newAIConfigFixture(t)
			mustWriteFile(t, filepath.Join(cfg.OpenCodeConfigDir, "AGENTS.md"), "old config\n", 0o644)
			if err := os.RemoveAll(path(cfg)); err != nil { t.Fatal(err) }
			if err := syncAIConfig(cfg, Runner{DryRun: dryRun}); err == nil { t.Fatalf("case %d dry=%t: expected source error", i, dryRun) }
			mustFileContents(t, filepath.Join(cfg.OpenCodeConfigDir, "AGENTS.md"), "old config\n")
			if _, err := os.Stat(filepath.Join(cfg.AIRuntimeDir, "ai-config-backups")); !os.IsNotExist(err) { t.Fatalf("case %d created backups", i) }
		}
	}
}

func TestAIConfigRollsBackActivation(t *testing.T) {
	root := t.TempDir(); first := filepath.Join(root, "live", "first"); second := filepath.Join(root, "live", "second"); stage := filepath.Join(root, "stage", "first")
	mustWriteFile(t, first, "old first\n", 0o644); mustWriteFile(t, second, "old second\n", 0o644); mustWriteFile(t, stage, "new first\n", 0o644)
	changes := []aiConfigChange{{destination:first, stagedPath:stage, backupPath:filepath.Join(root, "backup", "first")}, {destination:second, stagedPath:filepath.Join(root, "stage", "missing"), backupPath:filepath.Join(root, "backup", "second")}}
	if _, err := activateAIConfigChanges(changes); err == nil { t.Fatal("expected activation error") }
	mustFileContents(t, first, "old first\n"); mustFileContents(t, second, "old second\n")
}

func TestAIConfigCommandUsesDryRun(t *testing.T) {
	repo := t.TempDir(); home := t.TempDir(); t.Setenv("HOME", home); writeAIConfigSources(t, repo); mustWriteFile(t, filepath.Join(repo, "pde", "config", ".keep"), "", 0o644)
	out, errOut, err := executeInstallCmd(t, "install", "ai-config", "--repo-root", repo, "--dry-run")
	if err != nil { t.Fatalf("execute: %v", err) }; if errOut.Len() != 0 { t.Fatalf("stderr = %q", errOut.String()) }
	if !strings.Contains(out.String(), "DRY-RUN: stage AI config") { t.Fatalf("missing actions: %s", out.String()) }
	if _, err := os.Stat(filepath.Join(home, ".local", "share", "pde", "ai")); !os.IsNotExist(err) { t.Fatalf("dry-run changed runtime: %v", err) }
}

func TestAIConfigHelpListsTarget(t *testing.T) { out, _, err := executeInstallCmd(t, "install", "--help"); if err != nil { t.Fatal(err) }; if !strings.Contains(out.String(), "ai-config") { t.Fatal("help missing target") } }
func TestAIConfigErrorsListTarget(t *testing.T) { repo := t.TempDir(); t.Setenv("HOME", t.TempDir()); mustWriteFile(t, filepath.Join(repo, "pde", "config", ".keep"), "", 0o644); _, _, err := executeInstallCmd(t, "install", "missing", "--repo-root", repo); if err == nil || !strings.Contains(err.Error(), "ai-config") { t.Fatalf("unknown target error = %v", err) } }

func newAIConfigFixture(t *testing.T) *Config {
	t.Helper(); root := t.TempDir(); cfg := &Config{RepoRoot:root, HomeDir:filepath.Join(root,"home"), AIRepoDir:filepath.Join(root,"ai"), AIRuntimeDir:filepath.Join(root,"home",".local","share","pde","ai"), OpenCodeConfigDir:filepath.Join(root,"home",".config","opencode"), CodexConfigDir:filepath.Join(root,"home",".codex"), PiAgentDir:filepath.Join(root,"home",".pi","agent")}; writeAIConfigSources(t, root); return cfg
}
func writeAIConfigSources(t *testing.T, root string) {
	t.Helper(); mustWriteFile(t, filepath.Join(root,"ai","AGENTS.md"), "agents\n", 0o644); mustWriteFile(t, filepath.Join(root,"ai","opencode","agents","review.md"), "agent\n", 0o644); mustWriteFile(t, filepath.Join(root,"ai","opencode","commands","review.md"), "command\n", 0o644); mustWriteFile(t, filepath.Join(root,"ai","pi","agent","settings.json"), "{}\n", 0o644)
	for _, pair := range []struct{ root, name string }{{filepath.Join(root,"ai","codex","skills"),"create-plan"},{filepath.Join(root,"ai","skills"),"git-messages"},{filepath.Join(root,"ai","skills"),"self-improvement"}} { writeSkillFixture(t, pair.root, pair.name) }
	mustWriteFile(t, filepath.Join(root,"ai","skills","self-improvement","references","placement.md"), "placement\n", 0o644)
}
func writeSkillFixture(t *testing.T, dir, name string) { mustWriteFile(t, filepath.Join(dir,"SKILL.md"), "---\nname: "+name+"\ndescription: fixture\n---\n", 0o644) }
