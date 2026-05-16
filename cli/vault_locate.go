package main

import (
	"encoding/json"
	"io"
	"path/filepath"
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

func resolveVaultPaths(homeDir string, selector string) ([]string, error) {
	state, err := readVaultState(homeDir)
	if err != nil {
		return nil, err
	}
	return selectVaultPaths(state, selector)
}

func resolveVaults(homeDir string, lookup envLookup, selector string) ([]string, error) {
	return resolveVaultPaths(homeDir, selector)
}

func locateVaultMatches(vaults []string, filename, reference, query string) ([]string, error) {
	return findVaultNotes(vaults, filename, reference, query)
}
