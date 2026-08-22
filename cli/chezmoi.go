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

func ensureChezmoiTools(_ *Config, runner Runner) error {
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

		changed, err := chezmoiOpenCodeStatus(
			cfg,
			configPath,
			filepath.Join(stateDir, "state.boltdb"),
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
		return runner.Bash("apply Surveil OpenCode permission", chezmoiApplyScript(cfg, configPath))
	}

	if err := runner.MkdirAll("create chezmoi state dir", filepath.Dir(chezmoiStatePath(cfg)), 0o700); err != nil {
		return err
	}
	changed, err := chezmoiOpenCodeStatus(
		cfg,
		configPath,
		chezmoiStatePath(cfg),
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

func chezmoiOpenCodeStatus(
	cfg *Config,
	configPath string,
	statePath string,
	runner Runner,
) (bool, error) {
	statusArgs := append(
		chezmoiArgs(cfg, statePath),
		"status", "--path-style", "absolute", configPath,
	)
	cmd := exec.Command("chezmoi", statusArgs...)
	cmd.Env = append(os.Environ(), "PDE_SURVEIL_STATE_PATTERN="+surveilStatePattern(cfg.HomeDir))
	var stderr bytes.Buffer
	cmd.Stderr = io.MultiWriter(runner.stderr(), &stderr)
	output, err := cmd.Output()
	if err != nil {
		return false, commandOutputError("check Surveil OpenCode permission", stderr.String(), err)
	}
	if len(strings.TrimSpace(string(output))) == 0 {
		return false, nil
	}

	cmd = exec.Command("chezmoi", append(chezmoiArgs(cfg, statePath), "cat", configPath)...)
	cmd.Env = append(os.Environ(), "PDE_SURVEIL_STATE_PATTERN="+surveilStatePattern(cfg.HomeDir))
	stderr.Reset()
	cmd.Stderr = io.MultiWriter(runner.stderr(), &stderr)
	target, err := cmd.Output()
	if err != nil {
		return false, commandOutputError("render Surveil OpenCode permission", stderr.String(), err)
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
	args := append(
		chezmoiArgs(cfg, chezmoiStatePath(cfg)),
		"apply", "--force", "--no-tty", configPath,
	)
	return "PDE_SURVEIL_STATE_PATTERN=" +
		shellQuote(surveilStatePattern(cfg.HomeDir)) +
		" chezmoi " + shellJoin(args)
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
