package main

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"unicode"
)

type vaultNoteSearch struct {
	Filename  string
	Reference string
	Query     string
}

func findVaultNotes(vaults []string, search vaultNoteSearch) ([]string, error) {
	exactMatches := map[string]struct{}{}
	prefixMatches := map[string]struct{}{}
	normalizedMatches := map[string]struct{}{}
	normalizedReference := normalizeLookupTitle(search.Reference)
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

			if search.Filename != "" {
				if base == search.Filename || (filepath.Ext(search.Filename) == "" && stem == search.Filename) {
					exactMatches[path] = struct{}{}
				}
				return nil
			}

			if search.Reference != "" {
				rel, err := filepath.Rel(vault, path)
				if err != nil {
					return fmt.Errorf("relative path for %s: %w", path, err)
				}
				rel = filepath.ToSlash(rel)
				relStem := strings.TrimSuffix(rel, ".md")
				if search.Reference == base || search.Reference == stem || search.Reference == rel || search.Reference == relStem {
					exactMatches[path] = struct{}{}
					return nil
				}

				if isBackupNote(base) {
					return nil
				}

				if referenceMatchesIssuePrefix(search.Reference, base) || referenceMatchesIssuePrefix(search.Reference, stem) {
					prefixMatches[path] = struct{}{}
					return nil
				}

				if normalizedReference != "" && normalizeLookupTitle(stem) == normalizedReference {
					normalizedMatches[path] = struct{}{}
				}
				return nil
			}

			data, err := os.ReadFile(path)
			if err != nil {
				return err
			}
			if strings.Contains(string(data), search.Query) {
				exactMatches[path] = struct{}{}
			}
			return nil
		}); err != nil {
			return nil, fmt.Errorf("scan vault %s: %w", vault, err)
		}
	}

	for _, tier := range []map[string]struct{}{exactMatches, prefixMatches, normalizedMatches} {
		if len(tier) == 0 {
			continue
		}
		result := make([]string, 0, len(tier))
		for match := range tier {
			result = append(result, match)
		}
		sort.Strings(result)
		return result, nil
	}

	return nil, nil
}

func isBackupNote(name string) bool {
	return strings.Contains(strings.ToLower(name), ".backup-")
}

func referenceMatchesIssuePrefix(reference, candidate string) bool {
	reference = strings.ToLower(strings.TrimSpace(strings.TrimSuffix(reference, filepath.Ext(reference))))
	candidate = strings.ToLower(strings.TrimSpace(strings.TrimSuffix(candidate, filepath.Ext(candidate))))
	if reference == "" || candidate == "" {
		return false
	}
	if !strings.HasPrefix(candidate, reference) || len(candidate) <= len(reference) {
		return false
	}
	next := candidate[len(reference)]
	return next == ' ' || next == '-' || next == '_'
}

func stripLeadingIssueCode(value string) string {
	value = strings.TrimSpace(value)
	parts := strings.SplitN(value, " ", 2)
	if len(parts) == 2 {
		head := parts[0]
		hasLetter := false
		hasDigit := false
		for _, r := range head {
			switch {
			case unicode.IsLetter(r):
				hasLetter = true
			case unicode.IsDigit(r):
				hasDigit = true
			case r == '-':
			default:
				return value
			}
		}
		if hasLetter && hasDigit && strings.ContainsRune(head, '-') {
			return parts[1]
		}
	}
	return value
}

func normalizeVaultLookupValue(value string) string {
	value = strings.ToLower(strings.TrimSpace(strings.TrimSuffix(value, filepath.Ext(value))))
	if value == "" {
		return ""
	}

	var b strings.Builder
	lastSpace := false
	for _, r := range value {
		if unicode.IsLetter(r) || unicode.IsNumber(r) {
			b.WriteRune(r)
			lastSpace = false
			continue
		}
		if !lastSpace {
			b.WriteByte(' ')
			lastSpace = true
		}
	}
	return strings.TrimSpace(b.String())
}

func normalizeLookupTitle(value string) string {
	return normalizeVaultLookupValue(stripLeadingIssueCode(value))
}
