package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const (
	obsidianRepo = "https://github.com/obsidian-nvim/obsidian.nvim"
	nvmVersion   = "v0.40.0"
	nodeVersion  = "22"
)

func installObsidian(cfg *Config, runner Runner) error {
	managedConfigDir := filepath.Join(cfg.RepoRoot, "pde", "config", "nvim")
	resolvedConfigDir, err := filepath.EvalSymlinks(cfg.NvimConfigDir)
	if err != nil || resolvedConfigDir != managedConfigDir {
		return fmt.Errorf("PDE Neovim config is not installed at %s; run ./pde/pde minimal first", cfg.NvimConfigDir)
	}

	if err := installObsidianPlugin(filepath.Join(cfg.NvimConfigDir, "pack", "plugins", "start", "obsidian.nvim"), runner); err != nil {
		return err
	}
	if err := ensureNode22(cfg, runner); err != nil {
		return err
	}
	if err := installHeadlessRuntime(cfg, runner); err != nil {
		return err
	}

	return nil
}

func installObsidianPlugin(pluginDir string, runner Runner) error {
	if checkoutLooksSane(pluginDir) {
		return nil
	}

	return runner.Do("install obsidian.nvim", func() error {
		tmpDir := pluginDir + ".tmp"
		if err := runner.RemoveAll("remove stale obsidian.nvim temp dir", tmpDir); err != nil {
			return err
		}
		if err := runner.MkdirAll("create obsidian.nvim parent dir", filepath.Dir(pluginDir), 0o755); err != nil {
			return err
		}

		if err := runner.Bash("clone obsidian.nvim", fmt.Sprintf(
			"set -euo pipefail; git clone --depth=1 %s %s",
			shellQuote(obsidianRepo),
			shellQuote(tmpDir),
		)); err != nil {
			_ = os.RemoveAll(tmpDir)
			return err
		}

		if err := runner.RemoveAll("replace obsidian.nvim checkout", pluginDir); err != nil {
			_ = os.RemoveAll(tmpDir)
			return err
		}

		if err := runner.Rename("activate obsidian.nvim checkout", tmpDir, pluginDir); err != nil {
			_ = os.RemoveAll(tmpDir)
			return err
		}

		return nil
	})
}

func ensureNode22(cfg *Config, runner Runner) error {
	nvmScript := filepath.Join(cfg.HomeDir, ".nvm", "nvm.sh")
	if _, err := os.Stat(nvmScript); err != nil {
		if err := runner.Bash("install nvm", fmt.Sprintf(
			"curl -fsSL https://raw.githubusercontent.com/nvm-sh/nvm/%s/install.sh | bash",
			nvmVersion,
		)); err != nil {
			return err
		}
	}

	return runner.Bash("install Node 22", fmt.Sprintf(
		"set -euo pipefail; export NVM_DIR=%s; source %s; nvm install %s",
		shellQuote(filepath.Join(cfg.HomeDir, ".nvm")),
		shellQuote(nvmScript),
		shellQuote(nodeVersion),
	))
}

func installHeadlessRuntime(cfg *Config, runner Runner) error {
	runtimeBinary := filepath.Join(cfg.RuntimeDir, "bin", "ob")
	if runtimeLooksSane(cfg.RuntimeDir) && wrapperLooksSane(runtimeBinary) {
		return nil
	}

	return runner.Do("install obsidian-headless runtime", func() error {
		if err := runner.MkdirAll("create obsidian-headless runtime dir", cfg.RuntimeDir, 0o755); err != nil {
			return err
		}

		if err := runner.Bash("install obsidian-headless", fmt.Sprintf(
			"set -euo pipefail; export NVM_DIR=%s; source %s; nvm use %s >/dev/null; npm install --prefix %s obsidian-headless",
			shellQuote(filepath.Join(cfg.HomeDir, ".nvm")),
			shellQuote(filepath.Join(cfg.HomeDir, ".nvm", "nvm.sh")),
			shellQuote(nodeVersion),
			shellQuote(cfg.RuntimeDir),
		)); err != nil {
			return err
		}

		return installRuntimeWrapper(cfg.RuntimeDir, runner)
	})
}

func installRuntimeWrapper(runtimeDir string, runner Runner) error {
	wrapperPath := filepath.Join(runtimeDir, "bin", "ob")
	script := fmt.Sprintf(`#!/usr/bin/env bash
set -euo pipefail
export NVM_DIR="$HOME/.nvm"
source "$NVM_DIR/nvm.sh"
nvm use %s >/dev/null
exec %s "$@"
`, nodeVersion, shellQuote(filepath.Join(runtimeDir, "node_modules", ".bin", "ob")))

	if err := runner.MkdirAll("create obsidian-headless wrapper dir", filepath.Dir(wrapperPath), 0o755); err != nil {
		return err
	}

	return runner.WriteFile("write obsidian-headless wrapper", wrapperPath, []byte(script), 0o755)
}

func checkoutLooksSane(pluginDir string) bool {
	info, err := os.Stat(pluginDir)
	if err != nil || !info.IsDir() {
		return false
	}

	if _, err := os.Stat(filepath.Join(pluginDir, ".git")); err != nil {
		return false
	}
	if _, err := os.Stat(filepath.Join(pluginDir, "lua", "obsidian", "init.lua")); err != nil {
		return false
	}

	return true
}

func runtimeLooksSane(runtimeDir string) bool {
	info, err := os.Stat(runtimeDir)
	if err != nil || !info.IsDir() {
		return false
	}

	if _, err := os.Stat(filepath.Join(runtimeDir, "node_modules", ".bin", "ob")); err != nil {
		return false
	}

	return true
}

func wrapperLooksSane(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Mode()&0o111 != 0
}

func shellQuote(value string) string {
	if value == "" {
		return "''"
	}

	return "'" + strings.ReplaceAll(value, "'", `'"'"'`) + "'"
}
