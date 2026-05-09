package main

import (
	"bytes"
	"encoding/json"
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
