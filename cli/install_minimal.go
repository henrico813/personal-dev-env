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

var minimalInstallErrorMessages = map[minimalInstallErrorCode]string{
	minimalLegacyInstallerMissing:       "legacy minimal installer not found at %s",
	minimalLegacyInstallerNotExecutable: "legacy minimal installer is not executable: %s",
}

type minimalInstallError struct {
	Code    minimalInstallErrorCode
	Message string
	Err     error
}

func (e *minimalInstallError) Error() string { return e.Message }

func (e *minimalInstallError) Unwrap() error { return e.Err }

func newMinimalInstallError(code minimalInstallErrorCode, err error, args ...any) *minimalInstallError {
	return &minimalInstallError{Code: code, Message: fmt.Sprintf(minimalInstallErrorMessages[code], args...), Err: err}
}

type minimalInstallers struct {
	runLegacyBase   func(*Config, Runner) error
	installConfig   func(*Config, Runner) error
	installObsidian func(*Config, Runner) error
	installAITools  func(*Config, Runner) error
}

var defaultMinimalInstallers = minimalInstallers{
	runLegacyBase:   runLegacyMinimalBase,
	installConfig:   installConfig,
	installObsidian: installObsidian,
	installAITools:  installAITools,
}

func installMinimal(cfg *Config, runner Runner) error {
	installers := defaultMinimalInstallers
	if runner.DryRun {
		installers.installObsidian = func(cfg *Config, runner Runner) error {
			return installObsidianWithOptions(cfg, runner, obsidianInstallOptions{
				skipNvimPreflightOnDryRun: true,
			})
		}
	}
	return runMinimal(cfg, runner, installers)
}

func runMinimal(cfg *Config, runner Runner, installers minimalInstallers) error {
	steps := []struct {
		name string
		run  func() error
	}{
		{name: "legacy minimal base", run: func() error { return installers.runLegacyBase(cfg, runner) }},
		{name: "config", run: func() error { return installers.installConfig(cfg, runner) }},
		{name: "obsidian", run: func() error { return installers.installObsidian(cfg, runner) }},
		{name: "ai-tools", run: func() error { return installers.installAITools(cfg, runner) }},
	}

	for _, step := range steps {
		if err := step.run(); err != nil {
			return fmt.Errorf("%s: %w", step.name, err)
		}
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
