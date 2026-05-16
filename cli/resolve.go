package main

import (
	"fmt"
	"os"
	"strings"
)

func selectVaultPaths(state VaultState, selector string) ([]string, error) {
	selector = normalizeVaultSelector(selector)
	if selector == "" {
		selector = "default"
	}

	homeDir, err := os.UserHomeDir()
	if err != nil {
		return nil, err
	}

	mainPath, err := resolveShellPath(state.MainPath, homeDir)
	if err != nil {
		return nil, err
	}
	workPath, err := resolveShellPath(state.WorkPath, homeDir)
	if err != nil {
		return nil, err
	}

	switch selector {
	case "default":
		switch normalizeVaultSelector(state.Default) {
		case "main":
			return selectVaultPaths(VaultState{MainPath: mainPath, WorkPath: workPath, Default: "main"}, "main")
		case "work":
			return selectVaultPaths(VaultState{MainPath: mainPath, WorkPath: workPath, Default: "work"}, "work")
		default:
			return nil, fmt.Errorf("PDE_DEFAULT_VAULT must be set to main or work")
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

func normalizeVaultSelector(s string) string {
	return strings.ToLower(normalizeShellValue(s))
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
