package main

import (
	"bytes"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestLocateVaultMatchesMarkdownOnly(t *testing.T) {
	vault := t.TempDir()
	mustWriteFile(t, filepath.Join(vault, "note.md"), "needle", 0o644)
	mustWriteFile(t, filepath.Join(vault, "note.txt"), "needle", 0o644)
	mustWriteFile(t, filepath.Join(vault, "other.md"), "needle in markdown", 0o644)

	matches, err := findVaultNotes([]string{vault}, "note", "", "")
	if err != nil {
		t.Fatalf("locate filename: %v", err)
	}
	if len(matches) != 1 || matches[0] != filepath.Join(vault, "note.md") {
		t.Fatalf("unexpected filename matches: %#v", matches)
	}

	matches, err = findVaultNotes([]string{vault}, "", "", "needle")
	if err != nil {
		t.Fatalf("locate query: %v", err)
	}
	if len(matches) != 2 {
		t.Fatalf("unexpected query matches: %#v", matches)
	}
	for _, match := range matches {
		if filepath.Ext(match) != ".md" {
			t.Fatalf("expected markdown match, got %q", match)
		}
	}
}

func TestFindVaultNotesReferenceMatching(t *testing.T) {
	tests := []struct {
		name      string
		files     map[string]string
		reference string
		want      []string
	}{
		{
			name: "case-insensitive issue id",
			files: map[string]string{
				"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.md": "needle",
			},
			reference: "pdev-113",
			want:      []string{"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.md"},
		},
		{
			name: "normalized title",
			files: map[string]string{
				"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.md": "needle",
			},
			reference: "simplify ai vault resolution and markdown wrap",
			want:      []string{"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.md"},
		},
		{
			name: "backup ignored by default",
			files: map[string]string{
				"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.md":                   "one",
				"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.backup-2026-05-18.md": "two",
			},
			reference: "PDEV-113",
			want:      []string{"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.md"},
		},
		{
			name: "explicit backup reference",
			files: map[string]string{
				"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.backup-2026-05-18.md": "two",
			},
			reference: "PDEV-113 Simplify AI vault resolution and markdown wrap.backup-2026-05-18.md",
			want:      []string{"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.backup-2026-05-18.md"},
		},
		{
			name: "ambiguous normalized title",
			files: map[string]string{
				"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.md":   "one",
				"archive/PDEV-214 Simplify AI vault resolution and markdown wrap.md": "two",
			},
			reference: "simplify ai vault resolution and markdown wrap",
			want: []string{
				"archive/PDEV-214 Simplify AI vault resolution and markdown wrap.md",
				"plans/PDEV-113 Simplify AI vault resolution and markdown wrap.md",
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			vault := t.TempDir()
			for rel, content := range tt.files {
				mustWriteFile(t, filepath.Join(vault, rel), content, 0o644)
			}

			matches, err := findVaultNotes([]string{vault}, "", tt.reference, "")
			if err != nil {
				t.Fatalf("locate reference %q: %v", tt.reference, err)
			}

			want := make([]string, 0, len(tt.want))
			for _, rel := range tt.want {
				want = append(want, filepath.Join(vault, rel))
			}
			if !reflect.DeepEqual(matches, want) {
				t.Fatalf("unexpected matches: %#v", matches)
			}
		})
	}
}

func TestRunVaultLocateSelectorMainSearchesMainVault(t *testing.T) {
	homeDir := t.TempDir()
	mainVault := filepath.Join(homeDir, "main")
	workVault := filepath.Join(homeDir, "work")
	configJSON := filepath.Join(homeDir, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(configJSON), 0o755); err != nil {
		t.Fatalf("mkdir config parent: %v", err)
	}
	if err := os.MkdirAll(mainVault, 0o755); err != nil {
		t.Fatalf("mkdir main vault: %v", err)
	}
	if err := os.MkdirAll(workVault, 0o755); err != nil {
		t.Fatalf("mkdir work vault: %v", err)
	}
	mustWriteFile(t, configJSON, "{\n  \"main_vault\": \""+mainVault+"\",\n  \"work_vault\": \""+workVault+"\"\n}\n", 0o644)
	mustWriteFile(t, filepath.Join(mainVault, "main.md"), "needle", 0o644)
	mustWriteFile(t, filepath.Join(workVault, "work.md"), "needle", 0o644)

	var out bytes.Buffer
	if err := runVaultLocate(&out, homeDir, vaultLocateOptions{Vault: "main", Query: "needle"}); err != nil {
		t.Fatalf("run vault locate: %v", err)
	}
	if got := out.String(); got != filepath.Join(mainVault, "main.md")+"\n" {
		t.Fatalf("unexpected output %q", got)
	}
}

func TestRunVaultLocateRejectsWhitespaceOnlyQuery(t *testing.T) {
	var out bytes.Buffer
	if err := runVaultLocate(&out, t.TempDir(), vaultLocateOptions{Vault: "default", Query: "   "}); err == nil {
		t.Fatal("expected whitespace query to be rejected")
	}
}

func TestResolveVaultsDefaultRequiresExplicitSelector(t *testing.T) {
	homeDir := t.TempDir()
	configJSON := filepath.Join(homeDir, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(configJSON), 0o755); err != nil {
		t.Fatalf("mkdir config parent: %v", err)
	}
	mustWriteFile(t, configJSON, "{\n  \"main_vault\": \"/vaults/main\",\n  \"work_vault\": \"/vaults/work\"\n}\n", 0o644)

	_, err := resolveVaultPaths(homeDir, "default")
	if err == nil {
		t.Fatal("expected missing default vault to fail")
	}

	var vaultErr *vaultError
	if !errors.As(err, &vaultErr) {
		t.Fatalf("expected vaultError, got %T", err)
	}
	if vaultErr.Code != vaultDefaultNotConfigured {
		t.Fatalf("unexpected error code %v", vaultErr.Code)
	}
}
