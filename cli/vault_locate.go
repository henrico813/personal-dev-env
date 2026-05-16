package main

import (
	"encoding/json"
	"io"
	"path/filepath"
	"strings"
)


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

