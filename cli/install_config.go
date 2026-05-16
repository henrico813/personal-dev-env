package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

type configLink struct {
	src string
	dst string
}

func managedSharedConfigLinks(cfg *Config) []configLink {
	return []configLink{
		{src: filepath.Join(cfg.RepoRoot, "pde", "config", "zsh", "zshrc"), dst: filepath.Join(cfg.HomeDir, ".zshrc")},
		{src: filepath.Join(cfg.RepoRoot, "pde", "config", "zsh", "zsh_plugins.txt"), dst: filepath.Join(cfg.HomeDir, ".zsh_plugins.txt")},
		{src: filepath.Join(cfg.RepoRoot, "pde", "config", "tmux", "tmux.conf"), dst: filepath.Join(cfg.HomeDir, ".tmux.conf")},
		{src: filepath.Join(cfg.RepoRoot, "pde", "config", "p10k", "p10k.zsh"), dst: filepath.Join(cfg.HomeDir, ".p10k.zsh")},
		{src: filepath.Join(cfg.RepoRoot, "pde", "config", "bottom", "bottom.toml"), dst: filepath.Join(cfg.HomeDir, ".config", "bottom", "bottom.toml")},
	}
}

func installConfig(cfg *Config, runner Runner) error {
	links := managedSharedConfigLinks(cfg)
	if err := preflightManagedSharedConfigSources(links); err != nil {
		return err
	}

	existingLines, err := existingPDEPathsEnvLines(filepath.Join(cfg.PDEConfigDir, "config.json"))
	if err != nil {
		return err
	}

	if err := writePDEPathsEnv(cfg, existingLines, runner); err != nil {
		return err
	}
	if err := runner.RemoveAll("remove deprecated PDE paths.env", filepath.Join(cfg.PDEConfigDir, "paths.env")); err != nil {
		return err
	}

	for _, link := range links {
		if err := linkConfig(link.src, link.dst, runner); err != nil {
			return err
		}
	}

	return nil
}

func preflightManagedSharedConfigSources(links []configLink) error {
	for _, link := range links {
		if err := validateReadableRegularFile(link.src); err != nil {
			return fmt.Errorf("validate managed source %s: %w", link.src, err)
		}
	}
	return nil
}

func writePDEPathsEnv(cfg *Config, existingLines map[string]string, runner Runner) error {
	pathsEnv := filepath.Join(cfg.PDEConfigDir, "config.json")
	content, err := json.MarshalIndent(pdeJSONConfig{
		InstallPath:            cfg.RepoRoot,
		Profile:                existingLines["PDE_PROFILE"],
		MainVault:              existingLines["PDE_MAIN_VAULT"],
		WorkVault:              existingLines["PDE_WORK_VAULT"],
		DefaultVault:           existingLines[defaultVaultEnvKey],
		OpenCodeBaseURL:        existingLines["OPENCODE_BASE_URL"],
		OpenCodeInlineShimPort: existingLines["OPENCODE_INLINE_SHIM_PORT"],
		OpenCodeInlineModel:    existingLines["OPENCODE_INLINE_MODEL"],
	}, "", "  ")
	if err != nil {
		return err
	}
	content = append(content, '\n')

	if err := backupConfigInstallPath(pathsEnv, runner); err != nil {
		return err
	}
	if err := runner.MkdirAll("create PDE config dir", cfg.PDEConfigDir, 0o755); err != nil {
		return err
	}
	return runner.WriteFile("write PDE config.json", pathsEnv, content, 0o644)
}

func existingPDEPathsEnvLines(path string) (map[string]string, error) {
	info, err := os.Stat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return map[string]string{}, nil
		}
		return nil, fmt.Errorf("stat existing PDE config file %s: %w", path, err)
	}
	if !info.Mode().IsRegular() {
		return map[string]string{}, nil
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("read existing PDE config file %s: %w", path, err)
	}
	var cfg pdeJSONConfig
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("decode existing PDE config file %s: %w", path, err)
	}
	lines := map[string]string{}
	if cfg.Profile != "" {
		lines["PDE_PROFILE"] = cfg.Profile
	}
	if cfg.MainVault != "" {
		lines["PDE_MAIN_VAULT"] = cfg.MainVault
	}
	if cfg.WorkVault != "" {
		lines["PDE_WORK_VAULT"] = cfg.WorkVault
	}
	if cfg.DefaultVault != "" {
		lines[defaultVaultEnvKey] = cfg.DefaultVault
	}
	if cfg.OpenCodeBaseURL != "" {
		lines["OPENCODE_BASE_URL"] = cfg.OpenCodeBaseURL
	}
	if cfg.OpenCodeInlineShimPort != "" {
		lines["OPENCODE_INLINE_SHIM_PORT"] = cfg.OpenCodeInlineShimPort
	}
	if cfg.OpenCodeInlineModel != "" {
		lines["OPENCODE_INLINE_MODEL"] = cfg.OpenCodeInlineModel
	}
	return lines, nil
}
