package main

import (
	"path/filepath"
)

// installAITools owns the whole AI bootstrap so `pde install ai-tools` stays the only public entry point.
func installAITools(cfg *Config, runner Runner) error {
	for _, path := range []string{
		filepath.Join(cfg.OpenCodeConfigDir, "agents"),
		filepath.Join(cfg.OpenCodeConfigDir, "commands"),
		filepath.Join(cfg.OpenCodeConfigDir, "AGENTS.md"),
		filepath.Join(cfg.CodexConfigDir, "skills"),
		filepath.Join(cfg.CodexConfigDir, "AGENTS.md"),
		filepath.Join(cfg.PiAgentDir, "settings.json"),
		filepath.Join(cfg.PiAgentDir, "AGENTS.md"),
	} {
		if err := backupIfExists(path, runner); err != nil {
			return err
		}
	}

	plannerBin, err := buildPlannerBinary(cfg, runner)
	if err != nil {
		return err
	}
	shimBin, err := buildOpenCodeInlineShimBinary(cfg, runner)
	if err != nil {
		return err
	}

	if err := ensureNodeToolchain(cfg, runner); err != nil {
		return err
	}
	if err := installNodeTool(cfg, runner, "codex", "@openai/codex"); err != nil {
		return err
	}
	if err := installNodeTool(cfg, runner, "opencode", "opencode-ai"); err != nil {
		return err
	}
	if err := installNodeTool(cfg, runner, "pi", "@mariozechner/pi-coding-agent"); err != nil {
		return err
	}

	if err := installOpenCodeConfig(cfg, runner); err != nil {
		return err
	}
	if err := installCodexConfig(cfg, runner); err != nil {
		return err
	}
	if err := installPiConfig(cfg, runner); err != nil {
		return err
	}
	if err := backupPlannerLaunchers(cfg, runner); err != nil {
		return err
	}
	if err := backupOpenCodeInlineShimLaunchers(cfg, runner); err != nil {
		return err
	}
	if err := installPlannerLaunchers(cfg, plannerBin, runner); err != nil {
		return err
	}
	if err := installOpenCodeInlineShimLaunchers(cfg, shimBin, runner); err != nil {
		return err
	}
	if err := verifyPlannerLauncher(cfg, runner); err != nil {
		return err
	}
	if err := verifyOpenCodeInlineShimLauncher(cfg, runner); err != nil {
		return err
	}

	return verifyPiLauncher(cfg, runner)
}

func installOpenCodeConfig(cfg *Config, runner Runner) error {
	if err := syncTree(filepath.Join(cfg.AIRepoDir, "opencode", "agents"), filepath.Join(cfg.OpenCodeConfigDir, "agents"), runner); err != nil {
		return err
	}
	if err := syncTree(filepath.Join(cfg.AIRepoDir, "opencode", "commands"), filepath.Join(cfg.OpenCodeConfigDir, "commands"), runner); err != nil {
		return err
	}
	return copyFile(filepath.Join(cfg.AIRepoDir, "AGENTS.md"), filepath.Join(cfg.OpenCodeConfigDir, "AGENTS.md"), runner)
}

func installCodexConfig(cfg *Config, runner Runner) error {
	if err := syncTree(filepath.Join(cfg.AIRepoDir, "codex", "skills"), filepath.Join(cfg.CodexConfigDir, "skills"), runner); err != nil {
		return err
	}
	return copyFile(filepath.Join(cfg.AIRepoDir, "AGENTS.md"), filepath.Join(cfg.CodexConfigDir, "AGENTS.md"), runner)
}

func installPiConfig(cfg *Config, runner Runner) error {
	if err := syncTreeInto(filepath.Join(cfg.AIRepoDir, "pi", "agent"), cfg.PiAgentDir, runner); err != nil {
		return err
	}
	return copyFile(filepath.Join(cfg.AIRepoDir, "AGENTS.md"), filepath.Join(cfg.PiAgentDir, "AGENTS.md"), runner)
}
