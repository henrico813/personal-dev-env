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
	manifest := "[package]\nname = \"surveil\"\nversion = \"0.1.0\"\n"
	manifestPath := filepath.Join(root, "surveil", "Cargo.toml")
	if err := os.WriteFile(manifestPath, []byte(manifest), 0o644); err != nil {
		t.Fatalf("write surveil manifest: %v", err)
	}
	createChezmoiSource(t, root)
	if err := os.MkdirAll(filepath.Join(cfg.OpenCodeConfigDir, "agents"), 0o755); err != nil {
		t.Fatalf("mkdir managed agents dir: %v", err)
	}
	sharedSkill := filepath.Join(cfg.HomeDir, ".agents", "skills", "git-messages")
	if err := os.MkdirAll(sharedSkill, 0o755); err != nil {
		t.Fatalf("mkdir managed shared skill: %v", err)
	}

	var output bytes.Buffer
	runner := Runner{DryRun: true, Stdout: &output, Stderr: &output}

	if err := installAITools(cfg, runner); err != nil {
		t.Fatalf("install ai tools dry run: %v", err)
	}

	dryRun := output.String()
	cargo := strings.Index(dryRun, "DRY-RUN: verify cargo")
	chezmoi := strings.Index(dryRun, "DRY-RUN: verify chezmoi")
	jq := strings.Index(dryRun, "DRY-RUN: verify jq")
	backup := strings.Index(dryRun, "DRY-RUN: backup existing config")
	plannerBuild := strings.Index(dryRun, "DRY-RUN: build planner")
	shimBuild := strings.Index(dryRun, "DRY-RUN: build opencode inline shim")
	surveilBuild := strings.Index(dryRun, "DRY-RUN: build surveil")
	surveilLink := strings.Index(dryRun, "DRY-RUN: link surveil")
	surveilVerify := strings.Index(dryRun, "DRY-RUN: verify surveil")
	surveilPermission := strings.Index(dryRun, "DRY-RUN: apply Surveil OpenCode permission")
	vibe := strings.Index(dryRun, "DRY-RUN: install vibe")
	node := strings.Index(dryRun, "DRY-RUN: install Node "+aiNodeVersion)
	stagePi := strings.Index(dryRun, "DRY-RUN: activate pi runtime")
	stagedPiRuntime := filepath.Join(cfg.AIRuntimeDir, "pi") + ".tmp"

	requiredSteps := []int{
		cargo,
		chezmoi,
		jq,
		backup,
		plannerBuild,
		shimBuild,
		surveilBuild,
		surveilLink,
		surveilVerify,
		surveilPermission,
		vibe,
		node,
		stagePi,
	}
	for _, step := range requiredSteps {
		if step == -1 {
			t.Fatalf("missing expected dry-run output:\n%s", dryRun)
		}
	}
	if cargo > chezmoi || chezmoi > jq || jq > backup || jq > plannerBuild || jq > shimBuild || jq > surveilBuild {
		t.Fatalf("tool preflight should run before mutable work:\n%s", dryRun)
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
	if surveilVerify > surveilPermission {
		t.Fatalf("surveil permission should follow verification:\n%s", dryRun)
	}
	if vibe > node {
		t.Fatalf("vibe install should run before Node setup:\n%s", dryRun)
	}
	if node > stagePi {
		t.Fatalf("Node setup should run before Pi activation:\n%s", dryRun)
	}
	if !strings.Contains(dryRun, "@earendil-works/pi-coding-agent@latest") {
		t.Fatalf("dry-run should install maintained Pi package:\n%s", dryRun)
	}
	if !strings.Contains(dryRun, "npm install --prefix "+shellQuote(stagedPiRuntime)) {
		t.Fatalf("dry-run should install into staged Pi runtime:\n%s", dryRun)
	}
	if strings.Contains(dryRun, "@mariozechner/pi-coding-agent") {
		t.Fatalf("dry-run should not install deprecated Pi package:\n%s", dryRun)
	}
	if !strings.Contains(dryRun, "backup existing config ("+sharedSkill+" -> "+sharedSkill+".backup.") {
		t.Fatalf("dry-run should back up the managed shared skill:\n%s", dryRun)
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
	if err := os.MkdirAll(filepath.Join(cfg.AIRepoDir, "skills"), 0o755); err != nil {
		t.Fatalf("mkdir shared skills: %v", err)
	}

	requireFile(filepath.Join(cfg.AIRepoDir, "opencode", "commands", "create_plan.md"), "opencode create-plan\n")
	requireFile(filepath.Join(cfg.AIRepoDir, "opencode", "commands", "implement_plan.md"), "opencode implement-plan\n")
	requireFile(filepath.Join(cfg.AIRepoDir, "opencode", "commands", "cleanup_plan.md"), "opencode cleanup-plan\n")
	requireFile(filepath.Join(cfg.AIRepoDir, "codex", "skills", "create-plan", "SKILL.md"), "codex create-plan\n")
	requireFile(filepath.Join(cfg.AIRepoDir, "codex", "skills", "implement-plan", "SKILL.md"), "codex implement-plan\n")
	requireFile(filepath.Join(cfg.AIRepoDir, "codex", "skills", "cleanup-plan", "SKILL.md"), "codex cleanup-plan\n")
	requireFile(filepath.Join(cfg.AIRepoDir, "skills", "git-messages", "SKILL.md"), "shared git messages\n")
	requireFile(filepath.Join(cfg.HomeDir, ".agents", "skills", "other", "SKILL.md"), "other skill\n")

	if err := installOpenCodeConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install opencode config: %v", err)
	}
	if err := installCodexConfig(cfg, Runner{}); err != nil {
		t.Fatalf("install codex config: %v", err)
	}
	if err := installGitMessagesSkill(cfg, Runner{}); err != nil {
		t.Fatalf("install git messages skill: %v", err)
	}

	cases := map[string]string{
		filepath.Join(cfg.OpenCodeConfigDir, "commands", "create_plan.md"):        "opencode create-plan\n",
		filepath.Join(cfg.OpenCodeConfigDir, "commands", "implement_plan.md"):     "opencode implement-plan\n",
		filepath.Join(cfg.OpenCodeConfigDir, "commands", "cleanup_plan.md"):       "opencode cleanup-plan\n",
		filepath.Join(cfg.CodexConfigDir, "skills", "create-plan", "SKILL.md"):     "codex create-plan\n",
		filepath.Join(cfg.CodexConfigDir, "skills", "implement-plan", "SKILL.md"):  "codex implement-plan\n",
		filepath.Join(cfg.CodexConfigDir, "skills", "cleanup-plan", "SKILL.md"):    "codex cleanup-plan\n",
		filepath.Join(cfg.CodexConfigDir, "skills", "git-messages", "SKILL.md"):    "shared git messages\n",
		filepath.Join(cfg.HomeDir, ".agents", "skills", "git-messages", "SKILL.md"): "shared git messages\n",
		filepath.Join(cfg.HomeDir, ".agents", "skills", "other", "SKILL.md"):       "other skill\n",
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

const (
	policyStart = "### Ignored-file policy\n"
	policyEnd   = "\n### 3. Finish housekeeping\n"
)

func cleanupPolicy(t *testing.T, source, text string) string {
	t.Helper()
	if strings.Count(text, policyStart) != 1 {
		t.Fatalf("%s must contain one ignored-file policy section", source)
	}
	if strings.Count(text, policyEnd) != 1 {
		t.Fatalf("%s must contain one ignored-file policy terminator", source)
	}
	_, afterStart, found := strings.Cut(text, policyStart)
	if !found {
		t.Fatalf("%s missing ignored-file policy start", source)
	}
	policy, _, found := strings.Cut(afterStart, policyEnd)
	if !found {
		t.Fatalf("%s missing ignored-file policy terminator", source)
	}
	return policyStart + policy
}

func TestCleanupPlanPoliciesMatch(t *testing.T) {
	sources := []string{
		filepath.Join("..", "ai", "opencode", "commands", "cleanup_plan.md"),
		filepath.Join("..", "ai", "codex", "skills", "cleanup-plan", "SKILL.md"),
	}
	required := []string{
		"git status --porcelain=v1 --untracked-files=all --ignored=no",
		"git ls-files --others --ignored --exclude-standard --no-directory --full-name -z",
		"NUL-terminated",
		"`/` is a directory record",
		"Block sensitive paths before allowlisted output paths.",
		"Allow a non-sensitive file only when",
		"`*.key`, `*.pem`, `.env*`, `.sops`, or `secrets*`",
		"`pde/user-config.yml`",
		"the root `.surveil/`",
		"root `vibe/target/`",
		"root `surveil/target/`",
		"any `__pycache__/` directory",
		"Block every other ignored path.",
	}
	rejected := []string{
		"git status --porcelain=v1 --ignored\n",
		"Treat uncommitted or untracked implementation changes as a stop condition.",
		"Treat tracked changes, or untracked files in paths that the cleanup action would change, remove, or overwrite, as a stop condition.",
		"If you find uncommitted or unexplained work in an artifact that would be changed or removed",
		"Never remove a worktree that still contains tracked local changes or untracked files",
		"Never remove a worktree that still contains uncommitted or unexplained files.",
		".pytest_cache/",
	}
	policies := make([]string, len(sources))

	for i, source := range sources {
		data, err := os.ReadFile(source)
		if err != nil {
			t.Fatalf("read %s: %v", source, err)
		}
		text := string(data)
		policy := cleanupPolicy(t, source, text)
		for _, want := range required {
			if !strings.Contains(policy, want) {
				t.Errorf("%s policy missing %q", source, want)
			}
		}
		sensitive := strings.Index(policy, "Block sensitive paths before")
		allowlist := strings.Index(policy, "Allow a non-sensitive file only when")
		if sensitive == -1 || allowlist == -1 || sensitive > allowlist {
			t.Errorf("%s policy must check sensitive paths before allowlists", source)
		}
		for _, unwanted := range rejected {
			if strings.Contains(text, unwanted) {
				t.Errorf("%s contains %q", source, unwanted)
			}
		}
		policies[i] = policy
	}
	if policies[0] != policies[1] {
		t.Error("OpenCode and Codex ignored-file policies differ")
	}
}
