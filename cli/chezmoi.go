package main

import (
	"bytes"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"
)

const _chezmoiConfigPath = "/dev/null"

func ensureSurveilOpenCodeTools(runner Runner) error {
	for _, tool := range []string{"chezmoi", "jq"} {
		tool := tool
		if err := runner.Do("verify "+tool, func() error {
			cmd := exec.Command(tool, "--version")
			cmd.Stdout = runner.stdout()
			cmd.Stderr = runner.stderr()
			if err := cmd.Run(); err != nil {
				return fmt.Errorf("verify %s: %w", tool, err)
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
	env := surveilOpenCodeEnv(cfg)
	exists, mode, err := validateOpenCodeConfigPath(configPath)
	if err != nil {
		return err
	}

	if runner.DryRun {
		stateDir, err := os.MkdirTemp("", "pde-chezmoi-")
		if err != nil {
			return fmt.Errorf("create temporary chezmoi state: %w", err)
		}
		defer os.RemoveAll(stateDir)

		statePath := filepath.Join(stateDir, "state.boltdb")
		changed, err := chezmoiTargetContentChanged(
			cfg,
			configPath,
			statePath,
			env,
			runner,
		)
		if err != nil {
			return err
		}
		if !changed {
			return nil
		}
		if exists {
			backupScript := "cp -p " + shellQuote(configPath) + " " +
				shellQuote(configPath+".bak")
			if err := runner.Bash("backup OpenCode config", backupScript); err != nil {
				return err
			}
		}
		return applyChezmoiTarget(
			cfg,
			configPath,
			statePath,
			env,
			runner,
			"apply Surveil OpenCode permission",
		)
	}

	if err := runner.MkdirAll("create chezmoi state dir", filepath.Dir(chezmoiStatePath(cfg)), 0o700); err != nil {
		return err
	}
	statePath := chezmoiStatePath(cfg)
	changed, err := chezmoiTargetContentChanged(
		cfg,
		configPath,
		statePath,
		env,
		runner,
	)
	if err != nil {
		return err
	}
	if !changed {
		return nil
	}
	if exists {
		backupPath := fmt.Sprintf(
			"%s.bak.%s.%d",
			configPath,
			time.Now().UTC().Format("20060102_150405.000000000"),
			os.Getpid(),
		)
		backupScript := "cp -p " + shellQuote(configPath) + " " + shellQuote(backupPath)
		if err := runner.Bash("backup OpenCode config", backupScript); err != nil {
			return err
		}
	}
	if err := runner.MkdirAll("create OpenCode config dir", cfg.OpenCodeConfigDir, 0o755); err != nil {
		return err
	}
	if err := applyChezmoiTarget(
		cfg,
		configPath,
		statePath,
		env,
		runner,
		"apply Surveil OpenCode permission",
	); err != nil {
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

func chezmoiTargetContentChanged(
	cfg *Config,
	targetPath string,
	statePath string,
	env []string,
	runner Runner,
) (bool, error) {
	output, err := runChezmoi(
		cfg,
		statePath,
		env,
		runner,
		"check chezmoi target",
		"status",
		"--path-style",
		"absolute",
		targetPath,
	)
	if err != nil {
		return false, err
	}
	if len(strings.TrimSpace(string(output))) == 0 {
		return false, nil
	}

	target, err := runChezmoi(
		cfg,
		statePath,
		env,
		runner,
		"render chezmoi target",
		"cat",
		targetPath,
	)
	if err != nil {
		return false, err
	}
	current, err := os.ReadFile(targetPath)
	if os.IsNotExist(err) {
		return true, nil
	}
	if err != nil {
		return false, fmt.Errorf("read OpenCode config: %w", err)
	}
	return !bytes.Equal(current, target), nil
}

func applyChezmoiTarget(
	cfg *Config,
	targetPath string,
	statePath string,
	env []string,
	runner Runner,
	action string,
) error {
	return runner.Do(action, func() error {
		_, err := runChezmoi(
			cfg,
			statePath,
			env,
			runner,
			"apply chezmoi target",
			"apply",
			"--force",
			"--no-tty",
			targetPath,
		)
		return err
	})
}

func chezmoiArgs(cfg *Config, statePath string) []string {
	return []string{
		"--config", _chezmoiConfigPath,
		"--config-format", "toml",
		"--source", filepath.Join(cfg.RepoRoot, "chezmoi"),
		"--destination", cfg.HomeDir,
		"--persistent-state", statePath,
		"--color", "false",
	}
}

func runChezmoi(
	cfg *Config,
	statePath string,
	env []string,
	runner Runner,
	action string,
	args ...string,
) ([]byte, error) {
	commandArgs := append(chezmoiArgs(cfg, statePath), args...)
	cmd := exec.Command("chezmoi", commandArgs...)
	cmd.Env = env

	var stderr bytes.Buffer
	cmd.Stderr = io.MultiWriter(runner.stderr(), &stderr)
	output, err := cmd.Output()
	if err != nil {
		return nil, commandOutputError(action, stderr.String(), err)
	}
	return output, nil
}

func commandOutputError(action, output string, err error) error {
	output = strings.TrimSpace(output)
	if output == "" {
		return fmt.Errorf("%s: %w", action, err)
	}
	return fmt.Errorf("%s: %s: %w", action, output, err)
}

func chezmoiStatePath(cfg *Config) string {
	return filepath.Join(cfg.HomeDir, ".local", "state", "pde", "chezmoi.boltdb")
}

func surveilOpenCodeEnv(cfg *Config) []string {
	return replaceEnv(
		os.Environ(),
		"PDE_SURVEIL_STATE_PATTERN",
		surveilStatePattern(cfg.HomeDir),
	)
}

func replaceEnv(env []string, key, value string) []string {
	result := make([]string, 0, len(env)+1)
	for _, entry := range env {
		entryKey, _, _ := strings.Cut(entry, "=")
		if entryKey != key {
			result = append(result, entry)
		}
	}
	return append(result, key+"="+value)
}

func surveilStatePattern(homeDir string) string {
	stateHome := os.Getenv("XDG_STATE_HOME")
	if !filepath.IsAbs(stateHome) {
		stateHome = filepath.Join(homeDir, ".local", "state")
	}
	return filepath.Join(stateHome, "surveil", "**")
}
