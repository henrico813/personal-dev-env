package main

import (
	"fmt"
	"os"
	"path/filepath"
)

const (
	aiNodeVersion = "22"
	aiNVMVersion  = "v0.40.0"
)

// buildPlannerBinary builds the nested planner Go module into a PDE-owned runtime.
func buildPlannerBinary(cfg *Config, runner Runner) (string, error) {
	plannerDir := filepath.Join(cfg.AIRuntimeDir, "planner")
	plannerBin := filepath.Join(plannerDir, "planner")
	plannerModuleDir := filepath.Join(cfg.RepoRoot, "planner")

	if err := runner.MkdirAll("create planner runtime dir", plannerDir, 0o755); err != nil {
		return "", err
	}
	if err := runner.Bash("build planner", fmt.Sprintf(
		"set -euo pipefail; cd %s && go build -o %s ./main",
		shellQuote(plannerModuleDir),
		shellQuote(plannerBin),
	)); err != nil {
		return "", err
	}
	return plannerBin, nil
}

func backupPlannerLaunchers(cfg *Config, runner Runner) error {
	for _, path := range []string{
		filepath.Join(cfg.LocalBinDir, "planner"),
		filepath.Join(cfg.OpenCodeConfigDir, "bin", "planner"),
		filepath.Join(cfg.CodexConfigDir, "skills", "create-plan", "bin", "planner"),
	} {
		if err := backupIfExists(path, runner); err != nil {
			return err
		}
	}
	return nil
}

func installPlannerLaunchers(cfg *Config, plannerBin string, runner Runner) error {
	for _, dst := range []string{
		filepath.Join(cfg.LocalBinDir, "planner"),
		filepath.Join(cfg.OpenCodeConfigDir, "bin", "planner"),
		filepath.Join(cfg.CodexConfigDir, "skills", "create-plan", "bin", "planner"),
	} {
		if err := linkBinary(plannerBin, dst, runner); err != nil {
			return err
		}
	}
	return nil
}

func verifyPlannerLauncher(cfg *Config, runner Runner) error {
	return runner.Bash("verify planner", fmt.Sprintf(
		"set -euo pipefail; export PATH=%s:$PATH; planner help >/dev/null",
		shellQuote(cfg.LocalBinDir),
	))
}

func verifyPiLauncher(cfg *Config, runner Runner) error {
	return runner.Bash("verify pi", fmt.Sprintf(
		"set -euo pipefail; test -x %s; %s --help >/dev/null",
		shellQuote(filepath.Join(cfg.LocalBinDir, "pi")),
		shellQuote(filepath.Join(cfg.LocalBinDir, "pi")),
	))
}

// ensureNodeToolchain keeps the Node runtime stable across reboots and shell restarts.
func ensureNodeToolchain(cfg *Config, runner Runner) error {
	nvmDir := filepath.Join(cfg.HomeDir, ".nvm")
	nvmScript := filepath.Join(nvmDir, "nvm.sh")

	if _, err := os.Stat(nvmScript); err != nil {
		if err := runner.Bash("install nvm", fmt.Sprintf(
			"curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/%s/install.sh | bash",
			aiNVMVersion,
		)); err != nil {
			return err
		}
	}

	return runner.Bash("install Node 22", fmt.Sprintf(
		"set -euo pipefail; export NVM_DIR=%s; source %s; nvm install %s >/dev/null",
		shellQuote(nvmDir),
		shellQuote(nvmScript),
		shellQuote(aiNodeVersion),
	))
}

// installNodeTool installs an npm-backed binary into a PDE-owned runtime and writes a stable launcher.
func installNodeTool(cfg *Config, runner Runner, name, pkg string) error {
	runtimeDir := filepath.Join(cfg.AIRuntimeDir, name)
	nvmDir := filepath.Join(cfg.HomeDir, ".nvm")
	nvmScript := filepath.Join(nvmDir, "nvm.sh")

	if err := runner.MkdirAll("create AI runtime dir", runtimeDir, 0o755); err != nil {
		return err
	}
	if err := runner.Bash(fmt.Sprintf("install %s", name), fmt.Sprintf(
		"set -euo pipefail; export NVM_DIR=%s; source %s; nvm use %s >/dev/null; npm install --prefix %s %s",
		shellQuote(nvmDir),
		shellQuote(nvmScript),
		shellQuote(aiNodeVersion),
		shellQuote(runtimeDir),
		shellQuote(pkg+"@latest"),
	)); err != nil {
		return err
	}

	wrapperPath := filepath.Join(cfg.LocalBinDir, name)
	if err := runner.MkdirAll("create local bin", cfg.LocalBinDir, 0o755); err != nil {
		return err
	}

	wrapper := fmt.Sprintf(`#!/usr/bin/env bash
set -euo pipefail
export NVM_DIR=%s
source %s
nvm use %s >/dev/null
exec %s "$@"
`, shellQuote(nvmDir), shellQuote(nvmScript), shellQuote(aiNodeVersion), shellQuote(filepath.Join(runtimeDir, "node_modules", ".bin", name)))
	return runner.WriteFile("write "+name+" wrapper", wrapperPath, []byte(wrapper), 0o755)
}
