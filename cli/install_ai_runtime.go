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

// ensurePlanner builds the repo planner into a PDE-owned runtime and links it into ~/.local/bin.
func ensurePlanner(cfg *Config, runner Runner) error {
	plannerDir := filepath.Join(cfg.AIRuntimeDir, "planner")
	plannerBin := filepath.Join(plannerDir, "planner")

	if err := runner.MkdirAll("create planner runtime dir", plannerDir, 0o755); err != nil {
		return err
	}
	if err := runner.Bash("build planner", fmt.Sprintf(
		"set -euo pipefail; cd %s && go build -o %s ./planner/main",
		shellQuote(cfg.RepoRoot),
		shellQuote(plannerBin),
	)); err != nil {
		return err
	}
	return linkBinary(plannerBin, filepath.Join(cfg.LocalBinDir, "planner"), runner)
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
		"set -euo pipefail; export NVM_DIR=%s; source %s; nvm use %s >/dev/null; npm install --prefix %s %s@latest",
		shellQuote(nvmDir),
		shellQuote(nvmScript),
		shellQuote(aiNodeVersion),
		shellQuote(runtimeDir),
		shellQuote(pkg),
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
