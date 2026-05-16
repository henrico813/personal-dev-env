package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
)

const defaultVaultEnvKey = "PDE_DEFAULT_VAULT"

type VaultState struct {
	MainPath string
	WorkPath string
	Default  string
}

type pdeJSONConfig struct {
	InstallPath            string `json:"install_path"`
	Profile                string `json:"profile"`
	MainVault              string `json:"main_vault"`
	WorkVault              string `json:"work_vault"`
	DefaultVault           string `json:"default_vault"`
	OpenCodeBaseURL        string `json:"opencode_base_url"`
	OpenCodeInlineShimPort string `json:"opencode_inline_shim_port"`
	OpenCodeInlineModel    string `json:"opencode_inline_model"`
}

func readVaultState(homeDir string) (VaultState, error) {
	path := filepath.Join(homeDir, ".config", "pde", "config.json")
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return VaultState{}, nil
		}
		return VaultState{}, newVaultError(vaultReadConfigFailed, err, err)
	}
	var cfg pdeJSONConfig
	if err := json.Unmarshal(data, &cfg); err != nil {
		return VaultState{}, newVaultError(vaultReadConfigFailed, err, err)
	}
	if cfg.DefaultVault != "" && cfg.DefaultVault != "main" && cfg.DefaultVault != "work" {
		return VaultState{}, newVaultError(vaultInvalidPersistedSelector, nil, cfg.DefaultVault)
	}
	return VaultState{MainPath: cfg.MainVault, WorkPath: cfg.WorkVault, Default: cfg.DefaultVault}, nil
}

func writeVaultState(homeDir string, state VaultState) error {
	if state.Default != "" && state.Default != "main" && state.Default != "work" {
		return newVaultError(vaultInvalidSelector, nil, state.Default)
	}

	path := filepath.Join(homeDir, ".config", "pde", "config.json")
	content, err := os.ReadFile(path)
	if err != nil && !os.IsNotExist(err) {
		return newVaultError(vaultReadConfigFailed, err, err)
	}
	if err == nil {
		var existing pdeJSONConfig
		if err := json.Unmarshal(content, &existing); err != nil {
			return newVaultError(vaultReadConfigFailed, err, err)
		}
		if existing.DefaultVault != "" && existing.DefaultVault != "main" && existing.DefaultVault != "work" {
			return newVaultError(vaultInvalidPersistedSelector, nil, existing.DefaultVault)
		}
	}

	updated, err := writeHelper(string(content), state)
	if err != nil {
		return newVaultError(vaultReadConfigFailed, err, err)
	}

	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return newVaultError(vaultWriteConfigFailed, err, err)
	}
	if err := backupConfigInstallPath(path, Runner{}); err != nil {
		return newVaultError(vaultWriteConfigFailed, err, err)
	}
	if err := os.WriteFile(path, []byte(updated), 0o644); err != nil {
		return newVaultError(vaultWriteConfigFailed, err, err)
	}
	return nil
}

func writeHelper(content string, state VaultState) (string, error) {
	cfg := pdeJSONConfig{}
	if strings.TrimSpace(content) != "" {
		if err := json.Unmarshal([]byte(content), &cfg); err != nil {
			return "", err
		}
	}
	if state.MainPath != "" {
		cfg.MainVault = state.MainPath
	}
	if state.WorkPath != "" {
		cfg.WorkVault = state.WorkPath
	}
	if state.Default != "" {
		cfg.DefaultVault = state.Default
	}

	data, err := json.MarshalIndent(cfg, "", "  ")
	if err != nil {
		return "", err
	}
	if len(data) == 0 {
		return "", nil
	}
	return string(append(data, '\n')), nil
}
