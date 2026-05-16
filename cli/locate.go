package main

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
)

func findVaultNotes(vaults []string, filename, reference, query string) ([]string, error) {
	matches := map[string]struct{}{}
	for _, vault := range vaults {
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

			base := filepath.Base(path)
			stem := strings.TrimSuffix(base, filepath.Ext(base))

			if filename != "" {
				if base == filename || (filepath.Ext(filename) == "" && stem == filename) {
					matches[path] = struct{}{}
				}
				return nil
			}

			if reference != "" {
				rel, err := filepath.Rel(vault, path)
				if err != nil {
					return nil
				}
				rel = filepath.ToSlash(rel)
				relStem := strings.TrimSuffix(rel, ".md")
				if reference == base || reference == stem || reference == rel || reference == relStem {
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
