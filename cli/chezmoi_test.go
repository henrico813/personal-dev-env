package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestChezmoiApply(t *testing.T) {
	t.Setenv("PDE_SURVEIL_STATE_PATTERN", "/incorrect/inherited/path/**")
	cfg, configPath := chezmoiTestConfig(t)
	seed := `{
  "model": "provider/model",
  "permission": {
    "bash": {"*": "ask"},
    "external_directory": {"*": "ask", "/mnt/vault/**": "allow"}
  }
}`
	writeChezmoiTestConfig(t, configPath, seed, 0o600)

	if err := applySurveilOpenCodePermission(cfg, Runner{}); err != nil {
		t.Fatalf("apply permission: %v", err)
	}

	config := readChezmoiTestConfig(t, configPath)
	if bytes.Contains(config, []byte("/incorrect/inherited/path/**")) {
		t.Fatal("config contains inherited Surveil path")
	}
	pattern := filepath.Join(cfg.HomeDir, ".local", "state", "surveil", "**")
	if got := configString(t, config, "permission", "external_directory", pattern); got != "allow" {
		t.Fatalf("permission = %q, want allow", got)
	}
	if got := configString(t, config, "permission", "external_directory", "/mnt/vault/**"); got != "allow" {
		t.Fatalf("vault permission = %q, want allow", got)
	}
	if got := configString(t, config, "model"); got != "provider/model" {
		t.Fatalf("model = %q, want provider/model", got)
	}
}

func TestChezmoiInitialConfig(t *testing.T) {
	tests := []struct {
		giveName   string
		giveExists bool
		giveJSON   string
	}{
		{giveName: "missing"},
		{giveName: "empty", giveExists: true, giveJSON: " \n\t"},
	}
	for _, tt := range tests {
		t.Run(tt.giveName, func(t *testing.T) {
			cfg, configPath := chezmoiTestConfig(t)
			if tt.giveExists {
				writeChezmoiTestConfig(t, configPath, tt.giveJSON, 0o644)
			}

			if err := applySurveilOpenCodePermission(cfg, Runner{}); err != nil {
				t.Fatalf("apply permission: %v", err)
			}
			config := readChezmoiTestConfig(t, configPath)
			pattern := filepath.Join(cfg.HomeDir, ".local", "state", "surveil", "**")
			got := configString(t, config, "permission", "external_directory", pattern)
			if got != "allow" {
				t.Fatalf("permission = %q, want allow", got)
			}
		})
	}
}

func TestChezmoiBackup(t *testing.T) {
	cfg, configPath := chezmoiTestConfig(t)
	seed := []byte("{\n  \"model\": \"provider/model\"\n}\n")
	writeChezmoiTestConfig(t, configPath, string(seed), 0o600)

	if err := applySurveilOpenCodePermission(cfg, Runner{}); err != nil {
		t.Fatalf("apply permission: %v", err)
	}
	backups, err := filepath.Glob(configPath + ".bak.*")
	if err != nil || len(backups) != 1 {
		t.Fatalf("backups = %v, err = %v", backups, err)
	}
	backup, err := os.ReadFile(backups[0])
	if err != nil {
		t.Fatalf("read backup: %v", err)
	}
	if !bytes.Equal(backup, seed) {
		t.Fatalf("backup = %q, want %q", backup, seed)
	}
	info, err := os.Stat(configPath)
	if err != nil {
		t.Fatalf("stat config: %v", err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("mode = %o, want 600", got)
	}
}

func TestChezmoiRerun(t *testing.T) {
	cfg, configPath := chezmoiTestConfig(t)
	writeChezmoiTestConfig(t, configPath, `{}`, 0o644)

	if err := applySurveilOpenCodePermission(cfg, Runner{}); err != nil {
		t.Fatalf("first apply: %v", err)
	}
	first := readChezmoiTestConfig(t, configPath)
	if err := applySurveilOpenCodePermission(cfg, Runner{}); err != nil {
		t.Fatalf("second apply: %v", err)
	}
	second := readChezmoiTestConfig(t, configPath)
	if !bytes.Equal(first, second) {
		t.Fatalf("rerun changed config:\n%s\n%s", first, second)
	}
	backups, err := filepath.Glob(configPath + ".bak.*")
	if err != nil || len(backups) != 1 {
		t.Fatalf("backups = %v, err = %v", backups, err)
	}
}

func TestChezmoiStatePaths(t *testing.T) {
	tests := []struct {
		giveName  string
		giveState string
		wantPath  string
	}{
		{giveName: "default", wantPath: "/home/test/.local/state/surveil/**"},
		{giveName: "absolute", giveState: "/state", wantPath: "/state/surveil/**"},
		{giveName: "relative", giveState: "state", wantPath: "/home/test/.local/state/surveil/**"},
	}
	for _, tt := range tests {
		t.Run(tt.giveName, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", tt.giveState)
			if got := surveilStatePattern("/home/test"); got != tt.wantPath {
				t.Fatalf("path = %q, want %q", got, tt.wantPath)
			}
		})
	}
}

func TestChezmoiRejectsConfig(t *testing.T) {
	tests := []struct {
		giveName string
		giveJSON string
		wantErr  string
	}{
		{
			giveName: "malformed",
			giveJSON: `{`,
			wantErr:  "parse error",
		},
		{
			giveName: "array root",
			giveJSON: `[]`,
			wantErr:  "OpenCode config must be an object",
		},
		{
			giveName: "global deny",
			giveJSON: `{"permission":"deny"}`,
			wantErr:  "permission string cannot preserve a narrow exception",
		},
		{
			giveName: "invalid external",
			giveJSON: `{"permission":{"external_directory":[]}}`,
			wantErr:  "external_directory must be a string or object",
		},
	}
	for _, tt := range tests {
		t.Run(tt.giveName, func(t *testing.T) {
			cfg, configPath := chezmoiTestConfig(t)
			writeChezmoiTestConfig(t, configPath, tt.giveJSON, 0o644)
			err := applySurveilOpenCodePermission(cfg, Runner{})
			if err == nil {
				t.Fatal("apply error = nil, want error")
			}
			if !strings.Contains(err.Error(), tt.wantErr) {
				t.Fatalf("apply error = %q, want substring %q", err, tt.wantErr)
			}
			if got := string(readChezmoiTestConfig(t, configPath)); got != tt.giveJSON {
				t.Fatalf("config = %q, want %q", got, tt.giveJSON)
			}
		})
	}
}

func TestChezmoiRejectsSymlink(t *testing.T) {
	cfg, configPath := chezmoiTestConfig(t)
	target := filepath.Join(t.TempDir(), "opencode.json")
	writeChezmoiTestConfig(t, target, `{}`, 0o644)
	if err := os.MkdirAll(filepath.Dir(configPath), 0o755); err != nil {
		t.Fatalf("mkdir config: %v", err)
	}
	if err := os.Symlink(target, configPath); err != nil {
		t.Fatalf("symlink config: %v", err)
	}
	if err := applySurveilOpenCodePermission(cfg, Runner{}); err == nil {
		t.Fatal("apply error = nil, want error")
	}
}

func TestChezmoiDryRun(t *testing.T) {
	cfg, configPath := chezmoiTestConfig(t)
	seed := []byte(`{"model":"provider/model"}`)
	writeChezmoiTestConfig(t, configPath, string(seed), 0o644)
	var output bytes.Buffer

	if err := applySurveilOpenCodePermission(cfg, Runner{DryRun: true, Stdout: &output, Stderr: &output}); err != nil {
		t.Fatalf("dry run: %v", err)
	}
	if got := readChezmoiTestConfig(t, configPath); !bytes.Equal(got, seed) {
		t.Fatalf("config = %q, want %q", got, seed)
	}
	if !strings.Contains(output.String(), "DRY-RUN: backup OpenCode config") ||
		!strings.Contains(output.String(), "DRY-RUN: apply Surveil OpenCode permission") {
		t.Fatalf("missing dry-run actions:\n%s", output.String())
	}
}

func TestChezmoiDryRunConverged(t *testing.T) {
	cfg, configPath := chezmoiTestConfig(t)
	writeChezmoiTestConfig(t, configPath, `{}`, 0o644)
	if err := applySurveilOpenCodePermission(cfg, Runner{}); err != nil {
		t.Fatalf("apply permission: %v", err)
	}
	var output bytes.Buffer

	err := applySurveilOpenCodePermission(
		cfg,
		Runner{DryRun: true, Stdout: &output, Stderr: &output},
	)
	if err != nil {
		t.Fatalf("dry run: %v", err)
	}
	if output.Len() != 0 {
		t.Fatalf("dry-run output = %q, want empty", output.String())
	}
}

func chezmoiTestConfig(t *testing.T) (*Config, string) {
	t.Helper()
	repoRoot, err := filepath.Abs("..")
	if err != nil {
		t.Fatalf("resolve repo root: %v", err)
	}
	home := t.TempDir()
	t.Setenv("XDG_STATE_HOME", "")
	cfg := &Config{
		RepoRoot:          repoRoot,
		HomeDir:           home,
		OpenCodeConfigDir: filepath.Join(home, ".config", "opencode"),
	}
	return cfg, filepath.Join(cfg.OpenCodeConfigDir, "opencode.json")
}

func writeChezmoiTestConfig(t *testing.T, path, content string, mode os.FileMode) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatalf("mkdir config: %v", err)
	}
	if err := os.WriteFile(path, []byte(content), mode); err != nil {
		t.Fatalf("write config: %v", err)
	}
}

func readChezmoiTestConfig(t *testing.T, path string) []byte {
	t.Helper()
	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read config: %v", err)
	}
	return content
}

func createChezmoiSource(t *testing.T, repoRoot string) {
	t.Helper()
	sourcePath := filepath.Join("..", "chezmoi", "dot_config", "opencode", "modify_opencode.json")
	content, err := os.ReadFile(sourcePath)
	if err != nil {
		t.Fatalf("read chezmoi source: %v", err)
	}
	destinationPath := filepath.Join(
		repoRoot,
		"chezmoi",
		"dot_config",
		"opencode",
		"modify_opencode.json",
	)
	writeChezmoiTestConfig(t, destinationPath, string(content), 0o755)
}

func configString(t *testing.T, data []byte, keys ...string) string {
	t.Helper()
	var value any
	if err := json.Unmarshal(data, &value); err != nil {
		t.Fatalf("decode config: %v", err)
	}
	for _, key := range keys {
		object, ok := value.(map[string]any)
		if !ok {
			t.Fatalf("%v is not an object", keys)
		}
		value = object[key]
	}
	text, ok := value.(string)
	if !ok {
		t.Fatalf("%v = %#v, want string", keys, value)
	}
	return text
}
