package installer

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

// Migration must preserve existing settings and legacy vault paths.
func TestLegacyConfigMigratesSafely(t *testing.T) {
	home := t.TempDir()
	directory := filepath.Join(home, ".config", "pde")
	if err := os.MkdirAll(directory, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(directory, "config.json"), []byte("{\"other\":true}\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	legacy := "PDE_MAIN_VAULT='/vault/main'\nexport PDE_DEFAULT_VAULT=main\n"
	if err := os.WriteFile(filepath.Join(directory, "paths.env"), []byte(legacy), 0o644); err != nil {
		t.Fatal(err)
	}

	journal, err := migrateLegacyConfig(config{Home: home, RepoRoot: "/repo"})
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatal(err)
	}
	data, err := os.ReadFile(filepath.Join(directory, "config.json"))
	if err != nil {
		t.Fatal(err)
	}
	var got map[string]any
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatal(err)
	}
	want := map[string]any{
		"default_vault": "main",
		"install_path":  "/repo",
		"main_vault":    "/vault/main",
		"other":         true,
	}
	if len(got) != len(want) {
		t.Fatalf("config = %#v", got)
	}
	for key, value := range want {
		if got[key] != value {
			t.Errorf("config[%q] = %#v, want %#v", key, got[key], value)
		}
	}
}

// A valid empty JSON value must not crash migration.
func TestLegacyConfigHandlesNull(t *testing.T) {
	home := t.TempDir()
	path := filepath.Join(home, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte("null\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	journal, err := migrateLegacyConfig(config{Home: home, RepoRoot: "/repo"})
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatal(err)
	}
}

// A later failure must not delete the original config backup.
func TestLegacyConfigRollbackRestoresFile(t *testing.T) {
	home := t.TempDir()
	path := filepath.Join(home, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	original := []byte("{\"install_path\":\"/old\"}\n")
	if err := os.WriteFile(path, original, 0o644); err != nil {
		t.Fatal(err)
	}

	journal, err := migrateLegacyConfig(config{Home: home, RepoRoot: "/new"})
	if err != nil {
		t.Fatal(err)
	}
	if err := journal.Rollback(); err != nil {
		t.Fatal(err)
	}
	restored, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(restored, original) {
		t.Fatalf("restored config = %q, want %q", restored, original)
	}
}
