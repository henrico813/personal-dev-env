package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/spf13/cobra"
)

func TestRootCmdRegistersVaultLocate(t *testing.T) {
	root := newRootCmd()
	vault := findSubcommand(root, "vault")
	if vault == nil {
		t.Fatal("expected vault command to be registered")
	}
	if findSubcommand(vault, "locate") == nil {
		t.Fatal("expected vault locate command to be registered")
	}
}

func TestVaultLocateDefaultsToSelectorDefault(t *testing.T) {
	cmd := newVaultLocateCmd()
	flag := cmd.Flags().Lookup("vault")
	if flag == nil {
		t.Fatal("expected --vault flag")
	}
	if flag.DefValue != "default" {
		t.Fatalf("unexpected default %q", flag.DefValue)
	}
}

func TestVaultLocatePositionalReferenceLookup(t *testing.T) {
	clearVaultEnv(t)
	homeDir := t.TempDir()
	workVault := filepath.Join(homeDir, "work")
	if err := os.MkdirAll(filepath.Join(workVault, "projects", "alpha"), 0o755); err != nil {
		t.Fatalf("mkdir work vault: %v", err)
	}
	pathsEnv := filepath.Join(homeDir, ".config", "pde", "paths.env")
	if err := os.MkdirAll(filepath.Dir(pathsEnv), 0o755); err != nil {
		t.Fatalf("mkdir paths.env parent: %v", err)
	}
	mustWriteFile(t, pathsEnv, "export PDE_WORK_VAULT=\""+workVault+"\"\n", 0o644)
	mustWriteFile(t, filepath.Join(workVault, "projects", "alpha", "note.md"), "needle", 0o644)
	t.Setenv("HOME", homeDir)

	stdout, stderr, err := executeVaultLocate(t, "vault", "locate", "projects/alpha/note.md")
	if err != nil {
		t.Fatalf("execute locate: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
	if got := stdout.String(); got != filepath.Join(workVault, "projects", "alpha", "note.md")+"\n" {
		t.Fatalf("unexpected output %q", got)
	}
}

func TestVaultLocatePositionalReferenceRejectsQuery(t *testing.T) {
	clearVaultEnv(t)
	t.Setenv("HOME", t.TempDir())

	stdout, stderr, err := executeVaultLocate(t, "vault", "locate", "projects/alpha/note.md", "--query", "needle")
	if err == nil {
		t.Fatal("expected positional reference with --query to be rejected")
	}
	if stdout.Len() != 0 {
		t.Fatalf("expected no stdout, got %q", stdout.String())
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
}

func TestVaultLocateJSONUsageFailure(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	stdout, stderr, err := executeVaultLocate(t, "vault", "locate", "--json")
	if err != nil {
		t.Fatalf("execute locate: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
	result := mustDecodeVaultLocateResult(t, stdout.Bytes())
	if result.Status != "error" {
		t.Fatalf("unexpected status %q", result.Status)
	}
	if result.Error == "" {
		t.Fatal("expected error message in JSON output")
	}
}

func TestVaultLocateJSONConfigFailure(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	stdout, stderr, err := executeVaultLocate(t, "vault", "locate", "--json", "--query", "note")
	if err != nil {
		t.Fatalf("execute locate: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
	result := mustDecodeVaultLocateResult(t, stdout.Bytes())
	if result.Status != "error" {
		t.Fatalf("unexpected status %q", result.Status)
	}
	if result.Error == "" {
		t.Fatal("expected config error in JSON output")
	}
}

func clearVaultEnv(t *testing.T) {
	t.Helper()
	mainVault, mainVaultOK := os.LookupEnv("PDE_MAIN_VAULT")
	workVault, workVaultOK := os.LookupEnv("PDE_WORK_VAULT")
	if err := os.Unsetenv("PDE_MAIN_VAULT"); err != nil {
		t.Fatalf("unset PDE_MAIN_VAULT: %v", err)
	}
	if err := os.Unsetenv("PDE_WORK_VAULT"); err != nil {
		t.Fatalf("unset PDE_WORK_VAULT: %v", err)
	}
	t.Cleanup(func() {
		if mainVaultOK {
			_ = os.Setenv("PDE_MAIN_VAULT", mainVault)
		} else {
			_ = os.Unsetenv("PDE_MAIN_VAULT")
		}
		if workVaultOK {
			_ = os.Setenv("PDE_WORK_VAULT", workVault)
		} else {
			_ = os.Unsetenv("PDE_WORK_VAULT")
		}
	})
}

func executeVaultLocate(t *testing.T, args ...string) (*bytes.Buffer, *bytes.Buffer, error) {
	t.Helper()
	root := newRootCmd()
	stdout := &bytes.Buffer{}
	stderr := &bytes.Buffer{}
	root.SetOut(stdout)
	root.SetErr(stderr)
	root.SetArgs(args)
	return stdout, stderr, root.Execute()
}

func mustDecodeVaultLocateResult(t *testing.T, data []byte) vaultLocateResult {
	t.Helper()
	var result vaultLocateResult
	if err := json.Unmarshal(bytes.TrimSpace(data), &result); err != nil {
		t.Fatalf("decode json: %v\n%s", err, string(data))
	}
	return result
}

func findSubcommand(cmd interface{ Commands() []*cobra.Command }, name string) *cobra.Command {
	for _, sub := range cmd.Commands() {
		if sub.Name() == name {
			return sub
		}
	}
	return nil
}
