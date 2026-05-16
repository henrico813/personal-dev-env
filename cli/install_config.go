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
	configJSON := map[string]any{}
	if info, err := os.Stat(pathsEnv); err == nil && info.Mode().IsRegular() {
		configJSON, err = readPDEConfig(pathsEnv)
		if err != nil {
			return err
		}
	} else if err != nil && !os.IsNotExist(err) {
		return fmt.Errorf("stat existing PDE config file %s: %w", pathsEnv, err)
	}
	configJSON["install_path"] = cfg.RepoRoot
	if profile := existingLines["PDE_PROFILE"]; profile != "" {
		configJSON["profile"] = profile
	}
	if mainVault := existingLines["PDE_MAIN_VAULT"]; mainVault != "" {
		configJSON["main_vault"] = mainVault
	}
	if workVault := existingLines["PDE_WORK_VAULT"]; workVault != "" {
		configJSON["work_vault"] = workVault
	}
	if defaultVault := existingLines[defaultVaultEnvKey]; defaultVault != "" {
		configJSON["default_vault"] = defaultVault
	}
	if baseURL := existingLines["OPENCODE_BASE_URL"]; baseURL != "" {
		configJSON["opencode_base_url"] = baseURL
	}
	if port := existingLines["OPENCODE_INLINE_SHIM_PORT"]; port != "" {
		configJSON["opencode_inline_shim_port"] = port
	}
	if model := existingLines["OPENCODE_INLINE_MODEL"]; model != "" {
		configJSON["opencode_inline_model"] = model
	}

	content, err := json.MarshalIndent(configJSON, "", "  ")
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
	var cfg map[string]any
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("decode existing PDE config file %s: %w", path, err)
	}
	lines := map[string]string{}
	if profile, err := readConfigStringField(cfg, "profile"); err != nil {
		return nil, fmt.Errorf("decode existing PDE config file %s: %w", path, err)
	} else if profile != "" {
		lines["PDE_PROFILE"] = profile
	}
	if mainVault, err := readConfigStringField(cfg, "main_vault"); err != nil {
		return nil, fmt.Errorf("decode existing PDE config file %s: %w", path, err)
	} else if mainVault != "" {
		lines["PDE_MAIN_VAULT"] = mainVault
	}
	if workVault, err := readConfigStringField(cfg, "work_vault"); err != nil {
		return nil, fmt.Errorf("decode existing PDE config file %s: %w", path, err)
	} else if workVault != "" {
		lines["PDE_WORK_VAULT"] = workVault
	}
	if defaultVault, err := readConfigStringField(cfg, "default_vault"); err != nil {
		return nil, fmt.Errorf("decode existing PDE config file %s: %w", path, err)
	} else if defaultVault != "" {
		lines[defaultVaultEnvKey] = defaultVault
	}
	if baseURL, err := readConfigStringField(cfg, "opencode_base_url"); err != nil {
		return nil, fmt.Errorf("decode existing PDE config file %s: %w", path, err)
	} else if baseURL != "" {
		lines["OPENCODE_BASE_URL"] = baseURL
	}
	if port, err := readConfigStringField(cfg, "opencode_inline_shim_port"); err != nil {
		return nil, fmt.Errorf("decode existing PDE config file %s: %w", path, err)
	} else if port != "" {
		lines["OPENCODE_INLINE_SHIM_PORT"] = port
	}
	if model, err := readConfigStringField(cfg, "opencode_inline_model"); err != nil {
		return nil, fmt.Errorf("decode existing PDE config file %s: %w", path, err)
	} else if model != "" {
		lines["OPENCODE_INLINE_MODEL"] = model
	}
	return lines, nil
}
