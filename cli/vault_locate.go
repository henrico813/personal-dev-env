package main

import (
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
	Vault     string
	Filename  string
	Reference string
	Query     string
	JSON      bool
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

func normalizeVaultReference(s string) string {
	s = strings.TrimSpace(s)
	if s == "" {
		return ""
	}
	return filepath.ToSlash(filepath.Clean(s))
}

func resolveVaults(homeDir string, lookup envLookup, selector string) ([]string, error) {
	cfg, err := loadVaultConfig(homeDir, lookup)
	if err != nil {
		return nil, err
	}
	return resolveVaultRoots(cfg, selector)
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

func matchesVaultFilename(path, filename string) bool {
	base := filepath.Base(path)
	stem := strings.TrimSuffix(base, filepath.Ext(base))
	return base == filename || (filepath.Ext(filename) == "" && stem == filename)
}

func matchesVaultReference(path, vaultRoot, reference string) bool {
	reference = normalizeVaultReference(reference)
	if reference == "" {
		return false
	}
	base := filepath.Base(path)
	stem := strings.TrimSuffix(base, filepath.Ext(base))
	rel, err := filepath.Rel(vaultRoot, path)
	if err != nil {
		return false
	}
	rel = filepath.ToSlash(rel)
	relStem := strings.TrimSuffix(rel, ".md")
	return reference == base || reference == stem || reference == rel || reference == relStem
}

func locateVaultMatches(vaults []string, filename, reference, query string) ([]string, error) {
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
				if matchesVaultFilename(path, filename) {
					matches[path] = struct{}{}
				}
				return nil
			}
			if reference != "" {
				if matchesVaultReference(path, vault, reference) {
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
