package main

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestInstallAIToolsDryRunChecksCargoBeforeMutations(t *testing.T) {
	root := t.TempDir()
	cfg := &Config{
		RepoRoot:          root,
		HomeDir:           filepath.Join(root, "home"),
		LocalBinDir:       filepath.Join(root, "home", ".local", "bin"),
		AIRepoDir:         filepath.Join(root, "ai"),
		AIRuntimeDir:      filepath.Join(root, "home", ".local", "share", "pde", "ai"),
		OpenCodeConfigDir: filepath.Join(root, "home", ".config", "opencode"),
		CodexConfigDir:    filepath.Join(root, "home", ".codex"),
		PiAgentDir:        filepath.Join(root, "home", ".pi", "agent"),
	}

	if err := os.MkdirAll(filepath.Join(root, "surveil"), 0o755); err != nil {
		t.Fatalf("mkdir surveil dir: %v", err)
	}
	if err := os.WriteFile(filepath.Join(root, "surveil", "Cargo.toml"), []byte("[package]\nname = \"surveil\"\nversion = \"0.1.0\"\n"), 0o644); err != nil {
		t.Fatalf("write surveil manifest: %v", err)
	}
	if err := os.MkdirAll(filepath.Join(cfg.OpenCodeConfigDir, "agents"), 0o755); err != nil {
		t.Fatalf("mkdir managed agents dir: %v", err)
	}

	var output bytes.Buffer
	runner := Runner{DryRun: true, Stdout: &output, Stderr: &output}

	if err := installAITools(cfg, runner); err != nil {
		t.Fatalf("install ai tools dry run: %v", err)
	}

	dryRun := output.String()
	cargo := strings.Index(dryRun, "DRY-RUN: verify cargo")
	backup := strings.Index(dryRun, "DRY-RUN: backup existing config")
	plannerBuild := strings.Index(dryRun, "DRY-RUN: build planner")
	shimBuild := strings.Index(dryRun, "DRY-RUN: build opencode inline shim")
	surveilBuild := strings.Index(dryRun, "DRY-RUN: build surveil")
	surveilLink := strings.Index(dryRun, "DRY-RUN: link surveil")
	surveilVerify := strings.Index(dryRun, "DRY-RUN: verify surveil")
	vibe := strings.Index(dryRun, "DRY-RUN: install vibe")
	node := strings.Index(dryRun, "DRY-RUN: install Node 22")

	if cargo == -1 || backup == -1 || plannerBuild == -1 || shimBuild == -1 || surveilBuild == -1 || surveilLink == -1 || surveilVerify == -1 || vibe == -1 || node == -1 {
		t.Fatalf("missing expected dry-run output:\n%s", dryRun)
	}
	if cargo > backup || cargo > plannerBuild || cargo > shimBuild || cargo > surveilBuild {
		t.Fatalf("cargo preflight should run before mutable work:\n%s", dryRun)
	}
	if plannerBuild > shimBuild || shimBuild > surveilBuild {
		t.Fatalf("build steps should stay in planner/shim/surveil order:\n%s", dryRun)
	}
	if surveilBuild > vibe {
		t.Fatalf("surveil build should run before vibe install:\n%s", dryRun)
	}
	if surveilLink > surveilVerify {
		t.Fatalf("surveil link should run before verify:\n%s", dryRun)
	}
	if vibe > node {
		t.Fatalf("vibe install should run before Node setup:\n%s", dryRun)
	}
}

func TestInstallAIToolsSyncsPlanDocsIntoManagedConfigDirs(t *testing.T) {
	root := t.TempDir()
	cfg := &Config{
		RepoRoot:          root,
		HomeDir:           filepath.Join(root, "home"),
		LocalBinDir:       filepath.Join(root, "home", ".local", "bin"),
		AIRepoDir:         filepath.Join(root, "ai"),
		AIRuntimeDir:      filepath.Join(root, "home", ".local", "share", "pde", "ai"),
		OpenCodeConfigDir: filepath.Join(root, "home", ".config", "opencode"),
		CodexConfigDir:    filepath.Join(root, "home", ".codex"),
		PiAgentDir:        filepath.Join(root, "home", ".pi", "agent"),
	}

	requireFile := func(path, contents string) {
		t.Helper()
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatalf("mkdir %s: %v", path, err)
		}
		if err := os.WriteFile(path, []byte(contents), 0o644); err != nil {
			t.Fatalf("write %s: %v", path, err)
		}
	}

	requireFile(filepath.Join(cfg.AIRepoDir, "AGENTS.md"), "repo agents\n")

	if err := os.MkdirAll(filepath.Join(cfg.AIRepoDir, "opencode", "agents"), 0o755); err != nil {
		t.Fatalf("mkdir opencode agents: %v", err)
	}
	if err := os.MkdirAll(filepath.Join(cfg.AIRepoDir, "opencode", "commands"), 0o755); err != nil {
		t.Fatalf("mkdir opencode commands: %v", err)
	}
	if err := os.MkdirAll(filepath.Join(cfg.AIRepoDir, "codex", "skills"), 0o755); err != nil {
		t.Fatalf("mkdir codex skills: %v", err)
	}

	requireFile(filepath.Join(cfg.AIRepoDir, "opencode", "commands", "create_plan.md"), "opencode create-plan\n")
	requireFile(filepath.Join(cfg.AIRepoDir, "opencode", "commands", "implement_plan.md"), "opencode implement-plan\n")
	requireFile(filepath.Join(cfg.AIRepoDir, "codex", "skills", "create-plan", "SKILL.md"), "codex create-plan\n")
	requireFile(filepath.Join(cfg.AIRepoDir, "codex", "skills", "implement-plan", "SKILL.md"), "codex implement-plan\n")

	if err := installOpenCodeConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install opencode config: %v", err)
	}
	if err := installCodexConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install codex config: %v", err)
	}

	cases := map[string]string{
		filepath.Join(cfg.OpenCodeConfigDir, "commands", "create_plan.md"):       "opencode create-plan\n",
		filepath.Join(cfg.OpenCodeConfigDir, "commands", "implement_plan.md"):    "opencode implement-plan\n",
		filepath.Join(cfg.CodexConfigDir, "skills", "create-plan", "SKILL.md"):   "codex create-plan\n",
		filepath.Join(cfg.CodexConfigDir, "skills", "implement-plan", "SKILL.md"): "codex implement-plan\n",
	}

	for path, want := range cases {
		got, err := os.ReadFile(path)
		if err != nil {
			t.Fatalf("read %s: %v", path, err)
		}
		if string(got) != want {
			t.Fatalf("%s = %q, want %q", path, string(got), want)
		}
	}
}
