package main

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestReadConfig(t *testing.T) {
	tests := []struct {
		name    string
		seed    string
		want    VaultState
		wantErr bool
	}{
		{name: "missingcfg", want: VaultState{}},
		{
			name: "existingcfg",
			seed: "export PDE_MAIN_VAULT=\"/vaults/main\"\nexport PDE_WORK_VAULT=\"/vaults/work\"\nexport PDE_DEFAULT_VAULT=\"main\"\n",
			want: VaultState{MainPath: "/vaults/main", WorkPath: "/vaults/work", Default: "main"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			homeDir := t.TempDir()
			pathsEnv := filepath.Join(homeDir, ".config", "pde", "paths.env")
			if tt.seed != "" {
				if err := os.MkdirAll(filepath.Dir(pathsEnv), 0o755); err != nil {
					t.Fatalf("mkdir paths.env parent: %v", err)
				}
				mustWriteFile(t, pathsEnv, tt.seed, 0o644)
			}

			got, err := readVaultState(homeDir)
			if (err != nil) != tt.wantErr {
				t.Fatalf("read vault state error = %v wantErr %v", err, tt.wantErr)
			}
			if got != tt.want {
				t.Fatalf("read vault state = %#v want %#v", got, tt.want)
			}
		})
	}
}

func TestWriteConfig(t *testing.T) {
	tests := []struct {
		name        string
		seed        string
		state       VaultState
		wantContain []string
	}{
		{
			name:  "missingcfg",
			state: VaultState{MainPath: "/vaults/main", WorkPath: "/vaults/work", Default: "main"},
			wantContain: []string{
				"export PDE_MAIN_VAULT=\"/vaults/main\"",
				"export PDE_WORK_VAULT=\"/vaults/work\"",
				"export PDE_DEFAULT_VAULT=\"main\"",
			},
		},
		{
			name:  "existingcfg",
			seed:  "# existing\nexport OPENCODE_BASE_URL=\"http://127.0.0.1:4199\"\n",
			state: VaultState{MainPath: "/vaults/main", WorkPath: "/vaults/work", Default: "work"},
			wantContain: []string{
				"export OPENCODE_BASE_URL=\"http://127.0.0.1:4199\"",
				"export PDE_MAIN_VAULT=\"/vaults/main\"",
				"export PDE_WORK_VAULT=\"/vaults/work\"",
				"export PDE_DEFAULT_VAULT=\"work\"",
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			homeDir := t.TempDir()
			pathsEnv := filepath.Join(homeDir, ".config", "pde", "paths.env")
			if tt.seed != "" {
				if err := os.MkdirAll(filepath.Dir(pathsEnv), 0o755); err != nil {
					t.Fatalf("mkdir paths.env parent: %v", err)
				}
				mustWriteFile(t, pathsEnv, tt.seed, 0o644)
			}

			if err := writeVaultState(homeDir, tt.state); err != nil {
				t.Fatalf("write vault state: %v", err)
			}

			content := mustFileContents(t, pathsEnv, "")
			for _, want := range tt.wantContain {
				if !strings.Contains(content, want) {
					t.Fatalf("expected content to contain %q, got:\n%s", want, content)
				}
			}
		})
	}
}
