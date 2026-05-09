package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

type envLookup func(string) (string, bool)

type vaultLocateOptions struct {
	Vault    string
	Filename string
	Query    string
	JSON     bool
}

type vaultLocateResult struct {
	Status  string   `json:"status"`
	Path    string   `json:"path,omitempty"`
	Matches []string `json:"matches,omitempty"`
	Error   string   `json:"error,omitempty"`
}

func encodeVaultLocateJSON(out io.Writer, result vaultLocateResult) error {
	enc := json.NewEncoder(out)
	enc.SetEscapeHTML(false)
	return enc.Encode(result)
}

func normalizeQueryInput(s string) string {
	return strings.TrimSpace(s)
}

func resolveVaults(homeDir string, lookup envLookup, selector string) ([]string, error) {
	paths, err := loadVaultPaths(homeDir, lookup)
	if err != nil {
		return nil, err
	}

	mainVault := paths["PDE_MAIN_VAULT"]
	workVault := paths["PDE_WORK_VAULT"]

	switch selector {
	case "", "default", "any":
		var vaults []string
		for _, vault := range []string{mainVault, workVault} {
			if vault != "" {
				vaults = append(vaults, vault)
			}
		}
		if len(vaults) == 0 {
			return nil, fmt.Errorf("no vault configured; set PDE_MAIN_VAULT or PDE_WORK_VAULT in ~/.config/pde/paths.env or the environment")
		}
		return vaults, nil
	case "main":
		if mainVault == "" {
			return nil, fmt.Errorf("main vault not configured")
		}
		return []string{mainVault}, nil
	case "work":
		if workVault == "" {
			return nil, fmt.Errorf("work vault not configured")
		}
		return []string{workVault}, nil
	default:
		return nil, fmt.Errorf("invalid --vault value %q", selector)
	}
}

func loadVaultPaths(homeDir string, lookup envLookup) (map[string]string, error) {
	pathsEnv := filepath.Join(homeDir, ".config", "pde", "paths.env")
	fileValues, err := readVaultPathsEnv(pathsEnv)
	if err != nil {
		return nil, err
	}

	merged := map[string]string{}
	for _, key := range []string{"PDE_MAIN_VAULT", "PDE_WORK_VAULT"} {
		if value, ok := fileValues[key]; ok {
			merged[key] = value
		}
	}
	for _, key := range []string{"PDE_MAIN_VAULT", "PDE_WORK_VAULT"} {
		if value, ok := lookup(key); ok {
			merged[key] = value
		}
	}

	for key, value := range merged {
		resolved, err := resolveShellPath(value, homeDir)
		if err != nil {
			return nil, fmt.Errorf("resolve %s: %w", key, err)
		}
		merged[key] = resolved
	}
	return merged, nil
}

func readVaultPathsEnv(path string) (map[string]string, error) {
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
		if key != "PDE_MAIN_VAULT" && key != "PDE_WORK_VAULT" {
			continue
		}
		values[key] = strings.TrimSpace(value)
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("scan paths.env %s: %w", path, err)
	}
	return values, nil
}

func resolveShellPath(value, homeDir string) (string, error) {
	value = strings.TrimSpace(value)
	if value == "" {
		return "", nil
	}

	if len(value) >= 2 {
		if (value[0] == '"' && value[len(value)-1] == '"') || (value[0] == '\'' && value[len(value)-1] == '\'') {
			value = value[1 : len(value)-1]
		}
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

func validateVaultDir(path string) error {
	info, err := os.Stat(path)
	if err != nil {
		return fmt.Errorf("stat vault %s: %w", path, err)
	}
	if !info.IsDir() {
		return fmt.Errorf("vault %s is not a directory", path)
	}
	return nil
}

func locateVaultMatches(vaults []string, filename, query string) ([]string, error) {
	matches := map[string]struct{}{}
	for _, vault := range vaults {
		if err := validateVaultDir(vault); err != nil {
			return nil, err
		}
		if err := filepath.WalkDir(vault, func(path string, entry fs.DirEntry, walkErr error) error {
			if walkErr != nil {
				return walkErr
			}
			if entry.IsDir() {
				return nil
			}
			if filepath.Ext(path) != ".md" {
				return nil
			}
			if filename != "" {
				base := filepath.Base(path)
				stem := strings.TrimSuffix(base, filepath.Ext(base))
				if base == filename || (filepath.Ext(filename) == "" && stem == filename) {
					matches[path] = struct{}{}
				}
				return nil
			}

			data, err := os.ReadFile(path)
			if err != nil {
				return err
			}
			if strings.Contains(string(data), query) {
				matches[path] = struct{}{}
			}
			return nil
		}); err != nil {
			return nil, fmt.Errorf("scan vault %s: %w", vault, err)
		}
	}

	result := make([]string, 0, len(matches))
	for match := range matches {
		result = append(result, match)
	}
	sort.Strings(result)
	return result, nil
}
