package main

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

func resolveVaultPaths(homeDir, selector string) ([]string, error) {
	state, err := readVaultState(homeDir)
	if err != nil {
		return nil, err
	}
	return selectVaultPaths(state, selector)
}

func selectVaultPaths(state VaultState, selector string) ([]string, error) {
	selector = normalizeVaultSelector(selector)
	if selector == "" {
		selector = "default"
	}

	mainPath, err := normalizeVaultPath(state.MainPath)
	if err != nil {
		return nil, fmt.Errorf("normalize PDE_MAIN_VAULT: %w", err)
	}
	workPath, err := normalizeVaultPath(state.WorkPath)
	if err != nil {
		return nil, fmt.Errorf("normalize PDE_WORK_VAULT: %w", err)
	}
	defaultSelector := normalizeVaultSelector(state.Default)

	switch selector {
	case "default":
		switch defaultSelector {
		case "main":
			if err := requireVaultDir(mainPath, "PDE_MAIN_VAULT"); err != nil {
				return nil, err
			}
			return []string{mainPath}, nil
		case "work":
			if err := requireVaultDir(workPath, "PDE_WORK_VAULT"); err != nil {
				return nil, err
			}
			return []string{workPath}, nil
		default:
			return nil, newVaultError(vaultInvalidPersistedSelector, nil, defaultSelector)
		}
	case "main":
		if err := requireVaultDir(mainPath, "PDE_MAIN_VAULT"); err != nil {
			return nil, err
		}
		return []string{mainPath}, nil
	case "work":
		if err := requireVaultDir(workPath, "PDE_WORK_VAULT"); err != nil {
			return nil, err
		}
		return []string{workPath}, nil
	case "any":
		if err := requireVaultDir(mainPath, "PDE_MAIN_VAULT"); err != nil {
			return nil, err
		}
		if err := requireVaultDir(workPath, "PDE_WORK_VAULT"); err != nil {
			return nil, err
		}
		return []string{mainPath, workPath}, nil
	default:
		return nil, fmt.Errorf("invalid --vault value %q", selector)
	}
}

func normalizeShellValue(s string) string {
	s = strings.TrimSpace(s)
	if len(s) >= 2 && ((s[0] == '"' && s[len(s)-1] == '"') || (s[0] == '\'' && s[len(s)-1] == '\'')) {
		s = s[1 : len(s)-1]
	}
	return s
}

func normalizeVaultSelector(s string) string {
	return strings.ToLower(normalizeShellValue(s))
}

func normalizeVaultPath(path string) (string, error) {
	path = normalizeShellValue(path)
	if path == "" {
		return "", nil
	}
	if strings.HasPrefix(path, "$") {
		return "", fmt.Errorf("vault path must use ~ or an absolute path: %s", path)
	}
	if path == "~" {
		homeDir, err := os.UserHomeDir()
		if err != nil {
			return "", err
		}
		path = homeDir
	} else if strings.HasPrefix(path, "~/") {
		homeDir, err := os.UserHomeDir()
		if err != nil {
			return "", err
		}
		path = filepath.Join(homeDir, path[2:])
	}
	if !filepath.IsAbs(path) {
		abs, err := filepath.Abs(path)
		if err != nil {
			return "", err
		}
		path = abs
	}
	return path, nil
}

func requireVaultDir(path, key string) error {
	if path == "" {
		switch key {
		case "PDE_MAIN_VAULT":
			return newVaultError(vaultMainNotConfigured, nil)
		case "PDE_WORK_VAULT":
			return newVaultError(vaultWorkNotConfigured, nil)
		default:
			return newVaultError(vaultNoVaultConfigured, nil)
		}
	}
	info, err := os.Stat(path)
	if err != nil {
		return fmt.Errorf("stat vault %s: %w", path, err)
	}
	if !info.IsDir() {
		return fmt.Errorf("vault %s is not a directory", path)
	}
	return nil
}
