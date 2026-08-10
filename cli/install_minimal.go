package main

import (
	"fmt"
	"os"
	"path/filepath"
)

type minimalInstallErrorCode int

const (
	minimalLegacyInstallerMissing minimalInstallErrorCode = iota + 1
	minimalLegacyInstallerNotExecutable
)

func minimalInstallErrorMessage(code minimalInstallErrorCode) string {
	switch code {
	case minimalLegacyInstallerMissing:
		return "legacy minimal installer not found at %s"
	case minimalLegacyInstallerNotExecutable:
		return "legacy minimal installer is not executable: %s"
	default:
		return "unknown minimal installer error"
	}
}

type minimalInstallError struct {
	Code    minimalInstallErrorCode
	Message string
	Err     error
}

func (e *minimalInstallError) Error() string { return e.Message }

func (e *minimalInstallError) Unwrap() error { return e.Err }

func newMinimalInstallError(code minimalInstallErrorCode, err error, args ...any) *minimalInstallError {
	return &minimalInstallError{Code: code, Message: fmt.Sprintf(minimalInstallErrorMessage(code), args...), Err: err}
}

func installMinimal(cfg *Config, runner Runner) error {
	if err := runLegacyMinimalBase(cfg, runner); err != nil {
		return fmt.Errorf("legacy minimal base: %w", err)
	}
	if err := installConfig(cfg, runner); err != nil {
		return fmt.Errorf("config: %w", err)
	}
	if runner.DryRun {
		if err := installObsidianWithOptions(cfg, runner, obsidianInstallOptions{
			skipNvimPreflightOnDryRun: true,
		}); err != nil {
			return fmt.Errorf("obsidian: %w", err)
		}
	} else if err := installObsidian(cfg, runner); err != nil {
		return fmt.Errorf("obsidian: %w", err)
	}
	if err := installAITools(cfg, runner); err != nil {
		return fmt.Errorf("ai-tools: %w", err)
	}
	return nil
}

func runLegacyMinimalBase(cfg *Config, runner Runner) error {
	script, err := legacyInstallerPath(cfg)
	if err != nil {
		return err
	}
	return runner.Bash("run legacy minimal base", fmt.Sprintf(
		"set -euo pipefail; %s __legacy_minimal_base",
		shellQuote(script),
	))
}

func legacyInstallerPath(cfg *Config) (string, error) {
	script := filepath.Join(cfg.RepoRoot, "pde", "pde")
	info, err := os.Stat(script)
	if err != nil {
		if os.IsNotExist(err) {
			return "", newMinimalInstallError(minimalLegacyInstallerMissing, err, script)
		}
		return "", err
	}
	if info.Mode()&0o111 == 0 {
		return "", newMinimalInstallError(minimalLegacyInstallerNotExecutable, nil, script)
	}
	return script, nil
}
