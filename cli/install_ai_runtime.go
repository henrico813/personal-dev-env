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

func ensureSurveilSource(cfg *Config, runner Runner) error {
	surveilManifest := filepath.Join(cfg.RepoRoot, "surveil", "Cargo.toml")
	if _, err := os.Stat(surveilManifest); err != nil {
		return fmt.Errorf("surveil/Cargo.toml is required for `pde install ai-tools`; restore the surveil crate and re-run")
	}
	return nil
}

func buildSurveilBinary(cfg *Config, runner Runner) (string, error) {
	runtimeDir := filepath.Join(cfg.AIRuntimeDir, "surveil")
	targetDir := filepath.Join(runtimeDir, "target")
	surveilBin := filepath.Join(runtimeDir, "surveil")
	surveilSourceDir := filepath.Join(cfg.RepoRoot, "surveil")
	targetBinary := filepath.Join(targetDir, "release", "surveil")

	if err := runner.MkdirAll("create surveil runtime dir", runtimeDir, 0o755); err != nil {
		return "", err
	}
	if err := runner.Bash("build surveil", fmt.Sprintf(
		"set -euo pipefail; cd %s && cargo build --release --target-dir %s --bin surveil; install -m 0755 %s %s",
		shellQuote(surveilSourceDir),
		shellQuote(targetDir),
		shellQuote(targetBinary),
		shellQuote(surveilBin),
	)); err != nil {
		return "", err
	}
	return surveilBin, nil
}

func installSurveilLauncher(cfg *Config, surveilBin string, runner Runner) error {
	return linkBinary(surveilBin, filepath.Join(cfg.LocalBinDir, "surveil"), runner)
}

func verifySurveilLauncher(cfg *Config, runner Runner) error {
	return runner.Bash("verify surveil", fmt.Sprintf(
		"set -euo pipefail; export PATH=%s:$PATH; surveil --help >/dev/null",
		shellQuote(cfg.LocalBinDir),
	))
}

func buildOpenCodeInlineShimBinary(cfg *Config, runner Runner) (string, error) {
	shimDir := filepath.Join(cfg.AIRuntimeDir, "opencode-inline-shim")
	shimBin := filepath.Join(shimDir, "opencode-inline-shim")
	shimModuleDir := filepath.Join(cfg.RepoRoot, "cli")

	if err := runner.MkdirAll("create shim runtime dir", shimDir, 0o755); err != nil {
		return "", err
	}
	if err := runner.Bash("build opencode inline shim", fmt.Sprintf(
		"set -euo pipefail; cd %s && go build -o %s ./cmd/opencode-inline-shim",
		shellQuote(shimModuleDir),
		shellQuote(shimBin),
	)); err != nil {
		return "", err
	}
	return shimBin, nil
}

func vibeInstallRoot(cfg *Config) string {
	return filepath.Dir(cfg.LocalBinDir)
}

func vibeBinaryPath(cfg *Config) string {
	return filepath.Join(cfg.LocalBinDir, "vibe")
}

func ensureCargo(cfg *Config, runner Runner) error {
	return runner.Bash("verify cargo", fmt.Sprintf(
		"set -euo pipefail; export PATH=%s:$PATH; command -v cargo >/dev/null || { printf %s >&2; exit 1; }",
		shellQuote(filepath.Join(cfg.HomeDir, ".cargo", "bin")),
		shellQuote("cargo is required for `pde install ai-tools`; install Rust/Cargo and re-run\n"),
	))
}

func installVibe(cfg *Config, runner Runner) error {
	return runner.Bash("install vibe", fmt.Sprintf(
		"set -euo pipefail; export PATH=%s:$PATH; cargo install --path %s --root %s --force",
		shellQuote(filepath.Join(cfg.HomeDir, ".cargo", "bin")),
		shellQuote(filepath.Join(cfg.RepoRoot, "vibe")),
		shellQuote(vibeInstallRoot(cfg)),
	))
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

func backupOpenCodeInlineShimLaunchers(cfg *Config, runner Runner) error {
	return backupIfExists(filepath.Join(cfg.LocalBinDir, "opencode-inline-shim"), runner)
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

func installOpenCodeInlineShimLaunchers(cfg *Config, shimBin string, runner Runner) error {
	return linkBinary(shimBin, filepath.Join(cfg.LocalBinDir, "opencode-inline-shim"), runner)
}

func verifyPlannerLauncher(cfg *Config, runner Runner) error {
	return runner.Bash("verify planner", fmt.Sprintf(
		"set -euo pipefail; export PATH=%s:$PATH; planner help >/dev/null",
		shellQuote(cfg.LocalBinDir),
	))
}

func verifyOpenCodeInlineShimLauncher(cfg *Config, runner Runner) error {
	return runner.Bash("verify opencode inline shim", fmt.Sprintf(
		"set -euo pipefail; export PATH=%s:$PATH; opencode-inline-shim --help >/dev/null",
		shellQuote(cfg.LocalBinDir),
	))
}

func verifyVibeLauncher(cfg *Config, runner Runner) error {
	return runner.Bash("verify vibe", fmt.Sprintf(
		"set -euo pipefail; test -x %s; %s --help >/dev/null",
		shellQuote(vibeBinaryPath(cfg)),
		shellQuote(vibeBinaryPath(cfg)),
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
	if err := backupIfExists(wrapperPath, runner); err != nil {
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
