package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
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
	cfg, err := readPDEConfig(path)
	if err != nil {
		return VaultState{}, newVaultError(vaultReadConfigFailed, err, err)
	}
	mainPath, err := readConfigStringField(cfg, "main_vault")
	if err != nil {
		return VaultState{}, newVaultError(vaultReadConfigFailed, err, err)
	}
	workPath, err := readConfigStringField(cfg, "work_vault")
	if err != nil {
		return VaultState{}, newVaultError(vaultReadConfigFailed, err, err)
	}
	defaultVault, err := readConfigStringField(cfg, "default_vault")
	if err != nil {
		return VaultState{}, newVaultError(vaultReadConfigFailed, err, err)
	}
	if defaultVault != "" && defaultVault != "main" && defaultVault != "work" {
		return VaultState{}, newVaultError(vaultInvalidPersistedSelector, nil, defaultVault)
	}
	return VaultState{MainPath: mainPath, WorkPath: workPath, Default: defaultVault}, nil
}

func writeVaultState(homeDir string, state VaultState) error {
	if state.Default != "" && state.Default != "main" && state.Default != "work" {
		return newVaultError(vaultInvalidSelector, nil, state.Default)
	}

	path := filepath.Join(homeDir, ".config", "pde", "config.json")
	cfg, err := readPDEConfig(path)
	if err != nil {
		return newVaultError(vaultReadConfigFailed, err, err)
	}
	existingDefault, err := readConfigStringField(cfg, "default_vault")
	if err != nil {
		return newVaultError(vaultReadConfigFailed, err, err)
	}
	if existingDefault != "" && existingDefault != "main" && existingDefault != "work" {
		return newVaultError(vaultInvalidPersistedSelector, nil, existingDefault)
	}

	updated, err := writeHelper(cfg, state)
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

func readPDEConfig(path string) (map[string]any, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		if os.IsNotExist(err) {
			return map[string]any{}, nil
		}
		return nil, err
	}
	if len(bytes.TrimSpace(data)) == 0 {
		return map[string]any{}, nil
	}

	var cfg map[string]any
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, err
	}
	if cfg == nil {
		return map[string]any{}, nil
	}
	return cfg, nil
}

func readConfigStringField(cfg map[string]any, key string) (string, error) {
	value, ok := cfg[key]
	if !ok || value == nil {
		return "", nil
	}
	text, ok := value.(string)
	if !ok {
		return "", fmt.Errorf("config key %q must be a string", key)
	}
	return text, nil
}

func writeHelper(cfg map[string]any, state VaultState) (string, error) {
	if cfg == nil {
		cfg = map[string]any{}
	}
	if state.MainPath != "" {
		cfg["main_vault"] = state.MainPath
	}
	if state.WorkPath != "" {
		cfg["work_vault"] = state.WorkPath
	}
	if state.Default != "" {
		cfg["default_vault"] = state.Default
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
