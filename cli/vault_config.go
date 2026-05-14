package main

import (
	"bufio"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const defaultVaultEnvKey = "PDE_DEFAULT_VAULT"

type vaultConfig struct {
	MainPath        string
	WorkPath        string
	DefaultSelector string
}

func loadPersistedVaultConfig(homeDir string) (vaultConfig, error) {
	fileValues, err := readPathsEnvExports(pathsEnvPath(homeDir), []string{"PDE_MAIN_VAULT", "PDE_WORK_VAULT", defaultVaultEnvKey})
	if err != nil {
		return vaultConfig{}, newVaultError(vaultReadConfigFailed, err, err)
	}

	mainPath, err := resolveShellPath(fileValues["PDE_MAIN_VAULT"], homeDir)
	if err != nil {
		return vaultConfig{}, newVaultError(vaultReadConfigFailed, err, err)
	}
	workPath, err := resolveShellPath(fileValues["PDE_WORK_VAULT"], homeDir)
	if err != nil {
		return vaultConfig{}, newVaultError(vaultReadConfigFailed, err, err)
	}

	selector := normalizeVaultSelector(fileValues[defaultVaultEnvKey])
	if selector != "" && selector != "main" && selector != "work" {
		return vaultConfig{}, newVaultError(vaultInvalidPersistedSelector, nil, selector)
	}

	return vaultConfig{MainPath: mainPath, WorkPath: workPath, DefaultSelector: selector}, nil
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
	values, err := readPathsEnvExports(pathsEnvPath(homeDir), []string{defaultVaultEnvKey})
	if err != nil {
		return "", newVaultError(vaultReadConfigFailed, err, err)
	}

	selector := normalizeVaultSelector(values[defaultVaultEnvKey])
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

	cfg, err := loadPersistedVaultConfig(homeDir)
	if err != nil {
		return err
	}
	cfg.DefaultSelector = selector
	if _, err := resolveVaultRoots(cfg, "default"); err != nil {
		return err
	}

	return setPathsEnvExport(homeDir, defaultVaultEnvKey, selector)
}

func setPathsEnvExport(homeDir, key, value string) error {
	path := pathsEnvPath(homeDir)
	content, err := readPathsEnvFile(path)
	if err != nil {
		return newVaultError(vaultReadConfigFailed, err, err)
	}
	updated := upsertPathsEnvExport(content, key, value)
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
			if cfg.WorkPath != "" {
				return []string{cfg.WorkPath}, nil
			}
			if cfg.MainPath != "" {
				return []string{cfg.MainPath}, nil
			}
			return nil, newVaultError(vaultNoVaultConfigured, nil)
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

func pathsEnvPath(homeDir string) string {
	return filepath.Join(homeDir, ".config", "pde", "paths.env")
}

func readPathsEnvFile(path string) (string, error) {
	info, err := os.Stat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return "", nil
		}
		return "", err
	}
	if !info.Mode().IsRegular() {
		return "", nil
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return "", err
	}
	return string(data), nil
}

func readPathsEnvExports(path string, keys []string) (map[string]string, error) {
	allowed := map[string]struct{}{}
	for _, key := range keys {
		allowed[key] = struct{}{}
	}

	f, err := os.Open(path)
	if err != nil {
		if os.IsNotExist(err) {
			return map[string]string{}, nil
		}
		return nil, err
	}
	defer f.Close()

	values := map[string]string{}
	scanner := bufio.NewScanner(f)
	for scanner.Scan() {
		line := strings.TrimSpace(scanner.Text())
		if !strings.HasPrefix(line, "export ") {
			continue
		}
		key, value, ok := strings.Cut(strings.TrimPrefix(line, "export "), "=")
		if !ok {
			continue
		}
		key = strings.TrimSpace(key)
		if _, ok := allowed[key]; !ok {
			continue
		}
		values[key] = strings.TrimSpace(value)
	}
	if err := scanner.Err(); err != nil {
		return nil, err
	}
	return values, nil
}

func upsertPathsEnvExport(content, key, value string) string {
	if content == "" {
		return strings.Join([]string{
			"# Personal Dev Environment configuration",
			"# Auto-generated by pde installer",
			fmt.Sprintf("export %s=%q", key, value),
			"",
		}, "\n")
	}

	content = strings.ReplaceAll(content, "\r\n", "\n")
	lines := strings.Split(content, "\n")
	prefix := "export " + key + "="
	updated := make([]string, 0, len(lines)+1)
	replaced := false
	for _, line := range lines {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, prefix) {
			if replaced {
				continue
			}
			updated = append(updated, fmt.Sprintf("export %s=%q", key, value))
			replaced = true
			continue
		}
		updated = append(updated, line)
	}
	if !replaced {
		if len(updated) > 0 && updated[len(updated)-1] != "" {
			updated = append(updated, "")
		}
		updated = append(updated, fmt.Sprintf("export %s=%q", key, value))
	}
	if updated[len(updated)-1] != "" {
		updated = append(updated, "")
	}
	return strings.Join(updated, "\n")
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
