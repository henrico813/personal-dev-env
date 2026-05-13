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

func loadVaultConfig(homeDir string, lookup envLookup) (vaultConfig, error) {
	fileValues, err := readPathsEnvExports(pathsEnvPath(homeDir), []string{"PDE_MAIN_VAULT", "PDE_WORK_VAULT", defaultVaultEnvKey})
	if err != nil {
		return vaultConfig{}, err
	}

	rawMain := fileValues["PDE_MAIN_VAULT"]
	if value, ok := lookup("PDE_MAIN_VAULT"); ok {
		rawMain = value
	}
	rawWork := fileValues["PDE_WORK_VAULT"]
	if value, ok := lookup("PDE_WORK_VAULT"); ok {
		rawWork = value
	}
	rawDefault := fileValues[defaultVaultEnvKey]

	mainPath, err := resolveShellPath(rawMain, homeDir)
	if err != nil {
		return vaultConfig{}, fmt.Errorf("resolve PDE_MAIN_VAULT: %w", err)
	}
	workPath, err := resolveShellPath(rawWork, homeDir)
	if err != nil {
		return vaultConfig{}, fmt.Errorf("resolve PDE_WORK_VAULT: %w", err)
	}

	return vaultConfig{
		MainPath:        mainPath,
		WorkPath:        workPath,
		DefaultSelector: normalizeVaultSelector(rawDefault),
	}, nil
}

func storedDefaultVaultSelector(homeDir string) (string, error) {
	values, err := readPathsEnvExports(pathsEnvPath(homeDir), []string{defaultVaultEnvKey})
	if err != nil {
		return "", err
	}
	return normalizeVaultSelector(values[defaultVaultEnvKey]), nil
}

func setPathsEnvExport(homeDir, key, value string) error {
	path := pathsEnvPath(homeDir)
	content, err := readPathsEnvFile(path)
	if err != nil {
		return err
	}
	updated := upsertPathsEnvExport(content, key, value)
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	if err := backupConfigInstallPath(path, Runner{}); err != nil {
		return err
	}
	return os.WriteFile(path, []byte(updated), 0o644)
}

func resolveVaultRoots(cfg vaultConfig, selector string) ([]string, error) {
	selector = normalizeVaultSelector(selector)

	switch selector {
	case "", "default":
		switch cfg.DefaultSelector {
		case "main":
			if cfg.MainPath == "" {
				return nil, fmt.Errorf("default vault selector %q requires PDE_MAIN_VAULT", cfg.DefaultSelector)
			}
			return []string{cfg.MainPath}, nil
		case "work":
			if cfg.WorkPath == "" {
				return nil, fmt.Errorf("default vault selector %q requires PDE_WORK_VAULT", cfg.DefaultSelector)
			}
			return []string{cfg.WorkPath}, nil
		case "":
			if cfg.WorkPath != "" {
				return []string{cfg.WorkPath}, nil
			}
			if cfg.MainPath != "" {
				return []string{cfg.MainPath}, nil
			}
			return nil, fmt.Errorf("no vault configured; set PDE_MAIN_VAULT or PDE_WORK_VAULT in ~/.config/pde/paths.env or the environment")
		default:
			return nil, fmt.Errorf("invalid PDE_DEFAULT_VAULT %q; expected main or work", cfg.DefaultSelector)
		}
	case "any":
		var vaults []string
		for _, vault := range []string{cfg.MainPath, cfg.WorkPath} {
			if vault != "" {
				vaults = append(vaults, vault)
			}
		}
		if len(vaults) == 0 {
			return nil, fmt.Errorf("no vault configured; set PDE_MAIN_VAULT or PDE_WORK_VAULT in ~/.config/pde/paths.env or the environment")
		}
		return vaults, nil
	case "main":
		if cfg.MainPath == "" {
			return nil, fmt.Errorf("main vault not configured")
		}
		return []string{cfg.MainPath}, nil
	case "work":
		if cfg.WorkPath == "" {
			return nil, fmt.Errorf("work vault not configured")
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
		return "", fmt.Errorf("stat paths.env %s: %w", path, err)
	}
	if !info.Mode().IsRegular() {
		return "", nil
	}
	data, err := os.ReadFile(path)
	if err != nil {
		return "", fmt.Errorf("read paths.env %s: %w", path, err)
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
		return nil, fmt.Errorf("open paths.env %s: %w", path, err)
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
		return nil, fmt.Errorf("scan paths.env %s: %w", path, err)
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

	if len(value) >= 2 {
		if (value[0] == '"' && value[len(value)-1] == '"') || (value[0] == '\'' && value[len(value)-1] == '\'') {
			value = value[1 : len(value)-1]
		}
	}

	return value
}

func normalizeVaultSelector(s string) string {
	return strings.ToLower(normalizeShellValue(s))
}
