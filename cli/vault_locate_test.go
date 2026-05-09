package main

import (
	"bytes"
	"os"
	"path/filepath"
	"testing"
)

func TestLoadVaultPathsEnvOverridesFile(t *testing.T) {
	homeDir := t.TempDir()
	pathsEnv := filepath.Join(homeDir, ".config", "pde", "paths.env")
	if err := os.MkdirAll(filepath.Dir(pathsEnv), 0o755); err != nil {
		t.Fatalf("mkdir paths.env parent: %v", err)
	}
	fileMain := filepath.Join(homeDir, "file-main")
	fileWork := filepath.Join(homeDir, "file-work")
	mustWriteFile(t, pathsEnv, "export PDE_MAIN_VAULT=\""+fileMain+"\"\nexport PDE_WORK_VAULT=\""+fileWork+"\"\n", 0o644)

	envMain := filepath.Join(homeDir, "env-main")
	if err := os.MkdirAll(envMain, 0o755); err != nil {
		t.Fatalf("mkdir env vault: %v", err)
	}
	if err := os.MkdirAll(fileWork, 0o755); err != nil {
		t.Fatalf("mkdir file work vault: %v", err)
	}

	paths, err := loadVaultPaths(homeDir, func(key string) (string, bool) {
		if key == "PDE_MAIN_VAULT" {
			return envMain, true
		}
		return "", false
	})
	if err != nil {
		t.Fatalf("load vault paths: %v", err)
	}
	if got := paths["PDE_MAIN_VAULT"]; got != envMain {
		t.Fatalf("unexpected main vault %q want %q", got, envMain)
	}
	if got := paths["PDE_WORK_VAULT"]; got != fileWork {
		t.Fatalf("unexpected work vault %q want %q", got, fileWork)
	}
}

func TestResolveShellPathNormalizesShellStyleValues(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	cases := []struct {
		input string
		want  string
	}{
		{`"$HOME/vault"`, filepath.Join(homeDir, "vault")},
		{`'~/notes'`, filepath.Join(homeDir, "notes")},
	}

	for _, tc := range cases {
		got, err := resolveShellPath(tc.input, homeDir)
		if err != nil {
			t.Fatalf("resolve shell path %q: %v", tc.input, err)
		}
		if got != tc.want {
			t.Fatalf("resolve shell path %q = %q want %q", tc.input, got, tc.want)
		}
	}
}

func TestLocateVaultMatchesMarkdownOnly(t *testing.T) {
	vault := t.TempDir()
	mustWriteFile(t, filepath.Join(vault, "note.md"), "needle", 0o644)
	mustWriteFile(t, filepath.Join(vault, "note.txt"), "needle", 0o644)
	mustWriteFile(t, filepath.Join(vault, "other.md"), "needle in markdown", 0o644)

	matches, err := locateVaultMatches([]string{vault}, "note", "")
	if err != nil {
		t.Fatalf("locate filename: %v", err)
	}
	if len(matches) != 1 || matches[0] != filepath.Join(vault, "note.md") {
		t.Fatalf("unexpected filename matches: %#v", matches)
	}

	matches, err = locateVaultMatches([]string{vault}, "", "needle")
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

func TestRunVaultLocateSelectorMainSearchesMainVault(t *testing.T) {
	homeDir := t.TempDir()
	mainVault := filepath.Join(homeDir, "main")
	workVault := filepath.Join(homeDir, "work")
	if err := os.MkdirAll(mainVault, 0o755); err != nil {
		t.Fatalf("mkdir main vault: %v", err)
	}
	if err := os.MkdirAll(workVault, 0o755); err != nil {
		t.Fatalf("mkdir work vault: %v", err)
	}
	mustWriteFile(t, filepath.Join(mainVault, "main.md"), "needle", 0o644)
	mustWriteFile(t, filepath.Join(workVault, "work.md"), "needle", 0o644)

	var out bytes.Buffer
	if err := runVaultLocate(&out, homeDir, func(key string) (string, bool) {
		switch key {
		case "PDE_MAIN_VAULT":
			return mainVault, true
		case "PDE_WORK_VAULT":
			return workVault, true
		default:
			return "", false
		}
	}, vaultLocateOptions{Vault: "main", Query: "needle"}); err != nil {
		t.Fatalf("run vault locate: %v", err)
	}
	if got := out.String(); got != filepath.Join(mainVault, "main.md")+"\n" {
		t.Fatalf("unexpected output %q", got)
	}
}

func TestRunVaultLocateRejectsWhitespaceOnlyQuery(t *testing.T) {
	var out bytes.Buffer
	if err := runVaultLocate(&out, t.TempDir(), func(string) (string, bool) { return "", false }, vaultLocateOptions{Vault: "default", Query: "   "}); err == nil {
		t.Fatal("expected whitespace query to be rejected")
	}
}
