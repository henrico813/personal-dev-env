package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"io"
	"os"
	"path/filepath"
	"strings"
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
	defaultCmd := findSubcommand(vault, "default")
	if defaultCmd == nil {
		t.Fatal("expected vault default command to be registered")
	}
	if findSubcommand(defaultCmd, "get") == nil {
		t.Fatal("expected vault default get command to be registered")
	}
	if findSubcommand(defaultCmd, "set") == nil {
		t.Fatal("expected vault default set command to be registered")
	}
	mainCmd := findSubcommand(vault, "main")
	if mainCmd == nil || findSubcommand(mainCmd, "set") == nil {
		t.Fatal("expected vault main set command to be registered")
	}
	workCmd := findSubcommand(vault, "work")
	if workCmd == nil || findSubcommand(workCmd, "set") == nil {
		t.Fatal("expected vault work set command to be registered")
	}
	if findSubcommand(vault, "path") == nil {
		t.Fatal("expected vault path command to be registered")
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
	configJSON := filepath.Join(homeDir, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(configJSON), 0o755); err != nil {
		t.Fatalf("mkdir config parent: %v", err)
	}
	mustWriteFile(t, configJSON, "{\n  \"work_vault\": \""+workVault+"\",\n  \"default_vault\": \"work\"\n}\n", 0o644)
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
	clearVaultEnv(t)
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

func TestVaultDefaultGetPrintsUnsetWhenNotPersisted(t *testing.T) {
	clearVaultEnv(t)
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)

	stdout, stderr, err := executeVaultLocate(t, "vault", "default")
	if err != nil {
		t.Fatalf("execute vault default: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
	if got := stdout.String(); got != "unset\n" {
		t.Fatalf("unexpected output %q", got)
	}
}

func TestVaultDefaultSetPersistsSelectorAndGetPrintsIt(t *testing.T) {
	clearVaultEnv(t)
	homeDir := t.TempDir()
	configJSON := filepath.Join(homeDir, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(configJSON), 0o755); err != nil {
		t.Fatalf("mkdir config parent: %v", err)
	}
	mainVault := filepath.Join(homeDir, "main")
	if err := os.MkdirAll(mainVault, 0o755); err != nil {
		t.Fatalf("mkdir main vault: %v", err)
	}
	mustWriteFile(t, configJSON, "{\n  \"main_vault\": \""+mainVault+"\",\n  \"opencode_base_url\": \"http://127.0.0.1:4199\"\n}\n", 0o644)
	t.Setenv("HOME", homeDir)

	stdout, stderr, err := executeVaultLocate(t, "vault", "default", "set", "main")
	if err != nil {
		t.Fatalf("execute vault default set: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
	if got := stdout.String(); got != "main\n" {
		t.Fatalf("unexpected output %q", got)
	}

	content := mustFileContents(t, configJSON, "")
	if !strings.Contains(content, `"default_vault": "main"`) {
		t.Fatalf("expected default selector to be written, got:\n%s", content)
	}
	if !strings.Contains(content, `"opencode_base_url": "http://127.0.0.1:4199"`) {
		t.Fatalf("expected unrelated config to be preserved, got:\n%s", content)
	}

	stdout, stderr, err = executeVaultLocate(t, "vault", "default", "get")
	if err != nil {
		t.Fatalf("execute vault default get: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
	if got := stdout.String(); got != "main\n" {
		t.Fatalf("unexpected get output %q", got)
	}
}

func TestVaultDefaultSetRequiresPersistedTargetPath(t *testing.T) {
	clearVaultEnv(t)
	homeDir := t.TempDir()

	err := runVaultDefaultSet(io.Discard, homeDir, "main")
	if err == nil {
		t.Fatal("expected set main to fail without a persisted main vault path")
	}

	var vaultErr *vaultError
	if !errors.As(err, &vaultErr) {
		t.Fatalf("expected vaultError, got %T", err)
	}
	if vaultErr.Code != vaultMainNotConfigured {
		t.Fatalf("unexpected error code %v", vaultErr.Code)
	}
}

func TestLocateDefaultRejectsBrokenWork(t *testing.T) {
	clearVaultEnv(t)
	homeDir := t.TempDir()
	mainVault := filepath.Join(homeDir, "main")
	workVault := filepath.Join(homeDir, "work")
	if err := os.MkdirAll(mainVault, 0o755); err != nil {
		t.Fatalf("mkdir main vault: %v", err)
	}
	configJSON := filepath.Join(homeDir, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(configJSON), 0o755); err != nil {
		t.Fatalf("mkdir config parent: %v", err)
	}
	mustWriteFile(t, filepath.Join(mainVault, "main.md"), "needle", 0o644)
	mustWriteFile(t, configJSON, "{\n  \"main_vault\": \""+mainVault+"\",\n  \"work_vault\": \""+workVault+"\",\n  \"default_vault\": \"work\"\n}\n", 0o644)
	t.Setenv("HOME", homeDir)

	stdout, stderr, err := executeVaultLocate(t, "vault", "locate", "--json", "--query", "needle")
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
	if !strings.Contains(result.Error, workVault) {
		t.Fatalf("expected work vault error, got %q", result.Error)
	}
}

func TestLocateAnyStaysStrict(t *testing.T) {
	clearVaultEnv(t)
	homeDir := t.TempDir()
	mainVault := filepath.Join(homeDir, "main")
	workVault := filepath.Join(homeDir, "work")
	if err := os.MkdirAll(mainVault, 0o755); err != nil {
		t.Fatalf("mkdir main vault: %v", err)
	}
	configJSON := filepath.Join(homeDir, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(configJSON), 0o755); err != nil {
		t.Fatalf("mkdir config parent: %v", err)
	}
	mustWriteFile(t, filepath.Join(mainVault, "main.md"), "needle", 0o644)
	mustWriteFile(t, configJSON, "{\n  \"main_vault\": \""+mainVault+"\",\n  \"work_vault\": \""+workVault+"\"\n}\n", 0o644)
	t.Setenv("HOME", homeDir)

	stdout, stderr, err := executeVaultLocate(t, "vault", "locate", "--vault", "any", "--json", "--query", "needle")
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
	if !strings.Contains(result.Error, workVault) {
		t.Fatalf("expected work vault error, got %q", result.Error)
	}
}

func TestVaultMainSetAndPathCommands(t *testing.T) {
	clearVaultEnv(t)
	homeDir := t.TempDir()
	mainVault := filepath.Join(homeDir, "main")
	if err := os.MkdirAll(mainVault, 0o755); err != nil {
		t.Fatalf("mkdir main vault: %v", err)
	}
	configJSON := filepath.Join(homeDir, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(configJSON), 0o755); err != nil {
		t.Fatalf("mkdir config parent: %v", err)
	}
	mustWriteFile(t, configJSON, "{\n  \"opencode_base_url\": \"http://127.0.0.1:4199\"\n}\n", 0o644)
	t.Setenv("HOME", homeDir)

	stdout, stderr, err := executeVaultLocate(t, "vault", "main", "set", mainVault)
	if err != nil {
		t.Fatalf("execute vault main set: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
	if got := stdout.String(); got != mainVault+"\n" {
		t.Fatalf("unexpected output %q", got)
	}

	content := mustFileContents(t, configJSON, "")
	if !strings.Contains(content, `"main_vault": "`+mainVault+`"`) {
		t.Fatalf("expected main vault to be written, got:\n%s", content)
	}
	if !strings.Contains(content, `"opencode_base_url": "http://127.0.0.1:4199"`) {
		t.Fatalf("expected unrelated config to be preserved, got:\n%s", content)
	}

	stdout, stderr, err = executeVaultLocate(t, "vault", "path", "main")
	if err != nil {
		t.Fatalf("execute vault path main: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
	if got := stdout.String(); got != mainVault+"\n" {
		t.Fatalf("unexpected output %q", got)
	}
}

func TestVaultWorkSetAndPathCommands(t *testing.T) {
	clearVaultEnv(t)
	homeDir := t.TempDir()
	workVault := filepath.Join(homeDir, "work")
	if err := os.MkdirAll(workVault, 0o755); err != nil {
		t.Fatalf("mkdir work vault: %v", err)
	}
	configJSON := filepath.Join(homeDir, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(configJSON), 0o755); err != nil {
		t.Fatalf("mkdir config parent: %v", err)
	}
	mustWriteFile(t, configJSON, "{\n  \"opencode_base_url\": \"http://127.0.0.1:4199\"\n}\n", 0o644)
	t.Setenv("HOME", homeDir)

	stdout, stderr, err := executeVaultLocate(t, "vault", "work", "set", workVault)
	if err != nil {
		t.Fatalf("execute vault work set: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
	if got := stdout.String(); got != workVault+"\n" {
		t.Fatalf("unexpected output %q", got)
	}

	content := mustFileContents(t, configJSON, "")
	if !strings.Contains(content, `"work_vault": "`+workVault+`"`) {
		t.Fatalf("expected work vault to be written, got:\n%s", content)
	}
	if !strings.Contains(content, `"opencode_base_url": "http://127.0.0.1:4199"`) {
		t.Fatalf("expected unrelated config to be preserved, got:\n%s", content)
	}

	stdout, stderr, err = executeVaultLocate(t, "vault", "path", "work")
	if err != nil {
		t.Fatalf("execute vault path work: %v", err)
	}
	if stderr.Len() != 0 {
		t.Fatalf("expected no stderr, got %q", stderr.String())
	}
	if got := stdout.String(); got != workVault+"\n" {
		t.Fatalf("unexpected output %q", got)
	}
}

func clearVaultEnv(t *testing.T) {
	t.Helper()
	mainVault, mainVaultOK := os.LookupEnv("PDE_MAIN_VAULT")
	workVault, workVaultOK := os.LookupEnv("PDE_WORK_VAULT")
	defaultVault, defaultVaultOK := os.LookupEnv("PDE_DEFAULT_VAULT")
	if err := os.Unsetenv("PDE_MAIN_VAULT"); err != nil {
		t.Fatalf("unset PDE_MAIN_VAULT: %v", err)
	}
	if err := os.Unsetenv("PDE_WORK_VAULT"); err != nil {
		t.Fatalf("unset PDE_WORK_VAULT: %v", err)
	}
	if err := os.Unsetenv("PDE_DEFAULT_VAULT"); err != nil {
		t.Fatalf("unset PDE_DEFAULT_VAULT: %v", err)
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
		if defaultVaultOK {
			_ = os.Setenv("PDE_DEFAULT_VAULT", defaultVault)
		} else {
			_ = os.Unsetenv("PDE_DEFAULT_VAULT")
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
