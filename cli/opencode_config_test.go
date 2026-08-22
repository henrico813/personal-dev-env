package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

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

func TestSurveilPermissionMerge(t *testing.T) {
	pattern := "/home/test/.local/state/surveil/**"
	tests := []struct {
		name        string
		seed        string
		wantChanged bool
		wantErr     bool
		checks      map[string]string
	}{
		{
			name:        "missing permission",
			seed:        `{}`,
			wantChanged: true,
			checks:      map[string]string{"permission|external_directory|" + pattern: "allow"},
		},
		{
			name: "preserves sibling rules",
			seed: `{
  "model": "provider/model",
  "permission": {
    "bash": {"*": "ask", "git *": "allow"},
    "external_directory": {"*": "ask", "/mnt/vault/**": "allow"}
  }
}`,
			wantChanged: true,
			checks: map[string]string{
				"model":                           "provider/model",
				"permission|bash|*":               "ask",
				"permission|bash|git *":           "allow",
				"permission|external_directory|*": "ask",
				"permission|external_directory|/mnt/vault/**": "allow",
				"permission|external_directory|" + pattern:    "allow",
			},
		},
		{
			name:        "already allowed",
			seed:        `{"permission":{"external_directory":{"/home/test/.local/state/surveil/**":"allow"}}}`,
			wantChanged: false,
		},
		{
			name:        "expands external shorthand",
			seed:        `{"permission":{"external_directory":"ask"}}`,
			wantChanged: true,
			checks: map[string]string{
				"permission|external_directory|*":          "ask",
				"permission|external_directory|" + pattern: "allow",
			},
		},
		{
			name:        "global allow is sufficient",
			seed:        `{"permission":"allow"}`,
			wantChanged: false,
		},
		{
			name:    "rejects global deny",
			seed:    `{"permission":"deny"}`,
			wantErr: true,
		},
		{
			name:    "rejects malformed json",
			seed:    `{`,
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, changed, err := mergeSurveilOpenCodePermission([]byte(tt.seed), pattern)
			if (err != nil) != tt.wantErr {
				t.Fatalf("merge error = %v, wantErr %v", err, tt.wantErr)
			}
			if tt.wantErr {
				return
			}
			if changed != tt.wantChanged {
				t.Fatalf("changed = %v, want %v", changed, tt.wantChanged)
			}
			if !changed && string(got) != tt.seed {
				t.Fatalf("no-op changed bytes: %s", got)
			}
			for path, want := range tt.checks {
				if got := configString(t, got, strings.Split(path, "|")...); got != want {
					t.Fatalf("%s = %q, want %q", path, got, want)
				}
			}
		})
	}
}

func TestSurveilPermissionInstall(t *testing.T) {
	home := t.TempDir()
	t.Setenv("XDG_STATE_HOME", "")
	cfg := &Config{
		HomeDir:           home,
		OpenCodeConfigDir: filepath.Join(home, ".config", "opencode"),
	}
	configPath := filepath.Join(cfg.OpenCodeConfigDir, "opencode.json")
	seed := []byte("{\n  \"model\": \"provider/model\"\n}\n")
	if err := os.MkdirAll(cfg.OpenCodeConfigDir, 0o755); err != nil {
		t.Fatalf("mkdir config dir: %v", err)
	}
	if err := os.WriteFile(configPath, seed, 0o600); err != nil {
		t.Fatalf("write config: %v", err)
	}

	if err := installSurveilOpenCodePermission(cfg, Runner{}); err != nil {
		t.Fatalf("install permission: %v", err)
	}
	content, err := os.ReadFile(configPath)
	if err != nil {
		t.Fatalf("read config: %v", err)
	}
	pattern := filepath.Join(home, ".local", "state", "surveil", "**")
	if got := configString(t, content, "permission", "external_directory", pattern); got != "allow" {
		t.Fatalf("permission = %q, want allow", got)
	}
	info, err := os.Stat(configPath)
	if err != nil {
		t.Fatalf("stat config: %v", err)
	}
	if got := info.Mode().Perm(); got != 0o600 {
		t.Fatalf("mode = %o, want 600", got)
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

	if err := installSurveilOpenCodePermission(cfg, Runner{}); err != nil {
		t.Fatalf("repeat install: %v", err)
	}
	repeatedBackups, err := filepath.Glob(configPath + ".bak.*")
	if err != nil || len(repeatedBackups) != 1 {
		t.Fatalf("repeat backups = %v, err = %v", repeatedBackups, err)
	}
}

func TestSurveilPermissionDryRun(t *testing.T) {
	home := t.TempDir()
	t.Setenv("XDG_STATE_HOME", "")
	cfg := &Config{
		HomeDir:           home,
		OpenCodeConfigDir: filepath.Join(home, ".config", "opencode"),
	}
	configPath := filepath.Join(cfg.OpenCodeConfigDir, "opencode.json")
	seed := []byte(`{"model":"provider/model"}`)
	if err := os.MkdirAll(cfg.OpenCodeConfigDir, 0o755); err != nil {
		t.Fatalf("mkdir config dir: %v", err)
	}
	if err := os.WriteFile(configPath, seed, 0o644); err != nil {
		t.Fatalf("write config: %v", err)
	}

	var output bytes.Buffer
	runner := Runner{DryRun: true, Stdout: &output, Stderr: &output}
	if err := installSurveilOpenCodePermission(cfg, runner); err != nil {
		t.Fatalf("dry-run permission: %v", err)
	}
	content, err := os.ReadFile(configPath)
	if err != nil {
		t.Fatalf("read config: %v", err)
	}
	if !bytes.Equal(content, seed) {
		t.Fatalf("dry-run changed config: %q", content)
	}
	if !strings.Contains(output.String(), "DRY-RUN: backup existing config") ||
		!strings.Contains(output.String(), "DRY-RUN: write Surveil OpenCode permission") {
		t.Fatalf("missing dry-run actions:\n%s", output.String())
	}
}

func TestSurveilStatePattern(t *testing.T) {
	home := "/home/test"
	tests := []struct {
		name      string
		stateHome string
		want      string
	}{
		{name: "default", want: "/home/test/.local/state/surveil/**"},
		{name: "absolute xdg", stateHome: "/state", want: "/state/surveil/**"},
		{name: "relative xdg", stateHome: "state", want: "/home/test/.local/state/surveil/**"},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Setenv("XDG_STATE_HOME", tt.stateHome)
			if got := surveilStatePattern(home); got != tt.want {
				t.Fatalf("pattern = %q, want %q", got, tt.want)
			}
		})
	}
}
