package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

type vaultConfig struct {
	MainPath        string
	WorkPath        string
	DefaultSelector string
}

func loadPersistedVaultConfig(homeDir string) (vaultConfig, error) {
	state, err := readVaultState(homeDir)
	if err != nil {
		return vaultConfig{}, err
	}

	selector := normalizeVaultSelector(state.Default)
	if selector != "" && selector != "main" && selector != "work" {
		return vaultConfig{}, newVaultError(vaultInvalidPersistedSelector, nil, selector)
	}

	return vaultConfig{MainPath: state.MainPath, WorkPath: state.WorkPath, DefaultSelector: selector}, nil
}

func loadVaultConfig(homeDir string, lookup envLookup) (vaultConfig, error) {
	persisted, err := loadPersistedVaultConfig(homeDir)
	if err != nil {
		return vaultConfig{}, err
	}

	rawMain := persisted.MainPath
	if value, ok := lookup("PDE_MAIN_VAULT"); ok {
		rawMain = value
	}
	rawWork := persisted.WorkPath
	if value, ok := lookup("PDE_WORK_VAULT"); ok {
		rawWork = value
	}

	mainPath, err := resolveShellPath(rawMain, homeDir)
	if err != nil {
		return vaultConfig{}, newVaultError(vaultReadConfigFailed, err, err)
	}
	workPath, err := resolveShellPath(rawWork, homeDir)
	if err != nil {
		return vaultConfig{}, newVaultError(vaultReadConfigFailed, err, err)
	}

	return vaultConfig{MainPath: mainPath, WorkPath: workPath, DefaultSelector: persisted.DefaultSelector}, nil
}

func loadVaultPaths(homeDir string, lookup envLookup) (map[string]string, error) {
	cfg, err := loadVaultConfig(homeDir, lookup)
	if err != nil {
		return nil, err
	}

	paths := map[string]string{}
	if cfg.MainPath != "" {
		paths["PDE_MAIN_VAULT"] = cfg.MainPath
	}
	if cfg.WorkPath != "" {
		paths["PDE_WORK_VAULT"] = cfg.WorkPath
	}
	return paths, nil
}

func storedDefaultVaultSelector(homeDir string) (string, error) {
	state, err := readVaultState(homeDir)
	if err != nil {
		return "", err
	}

	selector := normalizeVaultSelector(state.Default)
	if selector != "" && selector != "main" && selector != "work" {
		return "", newVaultError(vaultInvalidPersistedSelector, nil, selector)
	}
	return selector, nil
}

func persistDefaultVaultSelector(homeDir, selector string) error {
	selector = normalizeVaultSelector(selector)
	if selector != "main" && selector != "work" {
		return newVaultError(vaultInvalidSelector, nil, selector)
	}

	state, err := readVaultState(homeDir)
	if err != nil {
		return err
	}
	cfg := vaultConfig{MainPath: state.MainPath, WorkPath: state.WorkPath, DefaultSelector: selector}
	if _, err := resolveVaultRoots(cfg, "default"); err != nil {
		return err
	}

	state.Default = selector
	return writeVaultState(homeDir, state)
}

func resolveVaultRoots(cfg vaultConfig, selector string) ([]string, error) {
	selector = normalizeVaultSelector(selector)

	switch selector {
	case "", "default":
		switch cfg.DefaultSelector {
		case "main":
			if cfg.MainPath == "" {
				return nil, newVaultError(vaultDefaultMainRequiresPath, nil, cfg.DefaultSelector)
			}
			return []string{cfg.MainPath}, nil
		case "work":
			if cfg.WorkPath == "" {
				return nil, newVaultError(vaultDefaultWorkRequiresPath, nil, cfg.DefaultSelector)
			}
			return []string{cfg.WorkPath}, nil
		case "":
			return resolveDefaultFallbackRoot(cfg)
		default:
			return nil, newVaultError(vaultInvalidPersistedSelector, nil, cfg.DefaultSelector)
		}
	case "any":
		var vaults []string
		for _, vault := range []string{cfg.MainPath, cfg.WorkPath} {
			if vault != "" {
				vaults = append(vaults, vault)
			}
		}
		if len(vaults) == 0 {
			return nil, newVaultError(vaultNoVaultConfigured, nil)
		}
		return vaults, nil
	case "main":
		if cfg.MainPath == "" {
			return nil, newVaultError(vaultMainNotConfigured, nil)
		}
		return []string{cfg.MainPath}, nil
	case "work":
		if cfg.WorkPath == "" {
			return nil, newVaultError(vaultWorkNotConfigured, nil)
		}
		return []string{cfg.WorkPath}, nil
	default:
		return nil, fmt.Errorf("invalid --vault value %q", selector)
	}
}

func resolveDefaultFallbackRoot(cfg vaultConfig) ([]string, error) {
	configured := false
	for _, vault := range []string{cfg.WorkPath, cfg.MainPath} {
		if vault == "" {
			continue
		}
		configured = true
		usable, err := isUsableDefaultFallbackRoot(vault)
		if err != nil {
			return nil, err
		}
		if usable {
			return []string{vault}, nil
		}
	}
	if !configured {
		return nil, newVaultError(vaultNoVaultConfigured, nil)
	}
	return nil, newVaultError(vaultDefaultNeedsUsablePath, nil)
}

func isUsableDefaultFallbackRoot(path string) (bool, error) {
	info, err := os.Stat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return false, nil
		}
		return false, fmt.Errorf("stat vault %s: %w", path, err)
	}
	return info.IsDir(), nil
}

func normalizeShellValue(value string) string {
	value = strings.TrimSpace(value)
	if value == "" {
		return ""
	}
	if len(value) >= 2 && ((value[0] == '"' && value[len(value)-1] == '"') || (value[0] == '\'' && value[len(value)-1] == '\'')) {
		value = value[1 : len(value)-1]
	}
	return value
}

func normalizeVaultSelector(s string) string {
	return strings.ToLower(normalizeShellValue(s))
}

func resolveShellPath(value, homeDir string) (string, error) {
	value = normalizeShellValue(value)
	if value == "" {
		return "", nil
	}
	value = os.ExpandEnv(value)
	if value == "~" {
		value = homeDir
	} else if strings.HasPrefix(value, "~/") {
		value = filepath.Join(homeDir, value[2:])
	}
	if value == "" {
		return "", nil
	}
	if !filepath.IsAbs(value) {
		abs, err := filepath.Abs(value)
		if err != nil {
			return "", err
		}
		value = abs
	}
	return value, nil
}
