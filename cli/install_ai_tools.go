package main

// installAITools installs AI runtimes before activating their configuration.
func installAITools(cfg *Config, runner Runner) error {
	if err := ensureCargo(cfg, runner); err != nil {
		return err
	}
	if err := ensureSurveilOpenCodeTools(runner); err != nil {
		return err
	}
	if err := ensureSurveilSource(cfg, runner); err != nil {
		return err
	}

	plannerBin, err := buildPlannerBinary(cfg, runner)
	if err != nil {
		return err
	}
	shimBin, err := buildOpenCodeInlineShimBinary(cfg, runner)
	if err != nil {
		return err
	}
	surveilBin, err := buildSurveilBinary(cfg, runner)
	if err != nil {
		return err
	}

	if err := installVibe(cfg, runner); err != nil {
		return err
	}
	if err := verifyVibeLauncher(cfg, runner); err != nil {
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
	if err := installNodeTool(cfg, runner, "pi", "@earendil-works/pi-coding-agent"); err != nil {
		return err
	}

	if err := installAIConfig(cfg, runner); err != nil {
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
	if err := installSurveilLauncher(cfg, surveilBin, runner); err != nil {
		return err
	}
	if err := verifyPlannerLauncher(cfg, runner); err != nil {
		return err
	}
	if err := verifyOpenCodeInlineShimLauncher(cfg, runner); err != nil {
		return err
	}
	if err := verifySurveilLauncher(cfg, runner); err != nil {
		return err
	}
	if err := applySurveilOpenCodePermission(cfg, runner); err != nil {
		return err
	}

	return verifyPiLauncher(cfg, runner)
}
