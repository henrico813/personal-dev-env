package main

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

const chezmoiConfigPath = "/dev/null"

func ensureChezmoiTools(_ *Config, runner Runner) error {
	for _, tool := range []string{"chezmoi", "jq"} {
		tool := tool
		if err := runner.Do("verify "+tool, func() error {
			if _, err := exec.LookPath(tool); err != nil {
				return fmt.Errorf("find %s: %w", tool, err)
			}
			return nil
		}); err != nil {
			return err
		}
	}
	return nil
}

func applySurveilOpenCodePermission(cfg *Config, runner Runner) error {
	configPath := filepath.Join(cfg.OpenCodeConfigDir, "opencode.json")
	exists, mode, err := validateOpenCodeConfigPath(configPath)
	if err != nil {
		return err
	}

	if runner.DryRun {
		if exists {
			if err := runner.Bash("backup OpenCode config", "cp -p "+shellQuote(configPath)+" "+shellQuote(configPath+".bak")); err != nil {
				return err
			}
		}
		return runner.Bash("apply Surveil OpenCode permission", chezmoiApplyScript(cfg, configPath))
	}

	if err := runner.MkdirAll("create chezmoi state dir", filepath.Dir(chezmoiStatePath(cfg)), 0o700); err != nil {
		return err
	}
	changed, err := chezmoiOpenCodeStatus(cfg, configPath, runner)
	if err != nil || !changed {
		return err
	}
	if exists {
		backupPath := fmt.Sprintf("%s.bak.%s.%d", configPath, time.Now().UTC().Format("20060102_150405.000000000"), os.Getpid())
		if err := runner.Bash("backup OpenCode config", "cp -p "+shellQuote(configPath)+" "+shellQuote(backupPath)); err != nil {
			return err
		}
	}
	if err := runner.MkdirAll("create OpenCode config dir", cfg.OpenCodeConfigDir, 0o755); err != nil {
		return err
	}
	if err := runner.Bash("apply Surveil OpenCode permission", chezmoiApplyScript(cfg, configPath)); err != nil {
		return err
	}
	if !exists {
		mode = 0o644
	}
	return runner.Do("restore OpenCode config mode", func() error {
		return os.Chmod(configPath, mode)
	})
}

func validateOpenCodeConfigPath(path string) (bool, os.FileMode, error) {
	info, err := os.Lstat(path)
	if os.IsNotExist(err) {
		return false, 0, nil
	}
	if err != nil {
		return false, 0, fmt.Errorf("stat OpenCode config: %w", err)
	}
	if !info.Mode().IsRegular() {
		return false, 0, fmt.Errorf("OpenCode config is not a regular file: %s", path)
	}
	return true, info.Mode().Perm(), nil
}

func chezmoiOpenCodeStatus(cfg *Config, configPath string, runner Runner) (bool, error) {
	cmd := exec.Command("chezmoi", append(chezmoiArgs(cfg), "status", "--path-style", "absolute", configPath)...)
	cmd.Env = append(os.Environ(), "PDE_SURVEIL_STATE_PATTERN="+surveilStatePattern(cfg.HomeDir))
	cmd.Stderr = runner.stderr()
	output, err := cmd.Output()
	if err != nil {
		return false, fmt.Errorf("check Surveil OpenCode permission: %w", err)
	}
	if len(strings.TrimSpace(string(output))) == 0 {
		return false, nil
	}

	cmd = exec.Command("chezmoi", append(chezmoiArgs(cfg), "cat", configPath)...)
	cmd.Env = append(os.Environ(), "PDE_SURVEIL_STATE_PATTERN="+surveilStatePattern(cfg.HomeDir))
	cmd.Stderr = runner.stderr()
	target, err := cmd.Output()
	if err != nil {
		return false, fmt.Errorf("render Surveil OpenCode permission: %w", err)
	}
	current, err := os.ReadFile(configPath)
	if os.IsNotExist(err) {
		return true, nil
	}
	if err != nil {
		return false, fmt.Errorf("read OpenCode config: %w", err)
	}
	return !bytes.Equal(current, target), nil
}

func chezmoiApplyScript(cfg *Config, configPath string) string {
	args := append(chezmoiArgs(cfg), "apply", "--force", "--no-tty", configPath)
	return "PDE_SURVEIL_STATE_PATTERN=" + shellQuote(surveilStatePattern(cfg.HomeDir)) + " chezmoi " + shellJoin(args)
}

func chezmoiArgs(cfg *Config) []string {
	return []string{
		"--config", chezmoiConfigPath,
		"--config-format", "toml",
		"--source", filepath.Join(cfg.RepoRoot, "chezmoi"),
		"--destination", cfg.HomeDir,
		"--persistent-state", chezmoiStatePath(cfg),
		"--color", "false",
	}
}

func chezmoiStatePath(cfg *Config) string {
	return filepath.Join(cfg.HomeDir, ".local", "state", "pde", "chezmoi.boltdb")
}

func shellJoin(args []string) string {
	quoted := make([]string, len(args))
	for i, arg := range args {
		quoted[i] = shellQuote(arg)
	}
	return strings.Join(quoted, " ")
}

func surveilStatePattern(homeDir string) string {
	stateHome := os.Getenv("XDG_STATE_HOME")
	if !filepath.IsAbs(stateHome) {
		stateHome = filepath.Join(homeDir, ".local", "state")
	}
	return filepath.Join(stateHome, "surveil", "**")
}
