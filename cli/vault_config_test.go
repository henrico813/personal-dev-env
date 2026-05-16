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
			seed: "{\n  \"main_vault\": \"/vaults/main\",\n  \"work_vault\": \"/vaults/work\",\n  \"default_vault\": \"main\"\n}\n",
			want: VaultState{MainPath: "/vaults/main", WorkPath: "/vaults/work", Default: "main"},
		},
		{
			name:    "invalidselector",
			seed:    "{\n  \"default_vault\": \"bogus\"\n}\n",
			wantErr: true,
		},
		{
			name:    "invalidjson",
			seed:    "{\n",
			wantErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			homeDir := t.TempDir()
			configJSON := filepath.Join(homeDir, ".config", "pde", "config.json")
			if tt.seed != "" {
				if err := os.MkdirAll(filepath.Dir(configJSON), 0o755); err != nil {
					t.Fatalf("mkdir config parent: %v", err)
				}
				mustWriteFile(t, configJSON, tt.seed, 0o644)
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
		name       string
		seed       string
		state      VaultState
		wantErr    bool
		wantChecks []string
		wantExact  string
	}{
		{
			name:  "missingcfg",
			state: VaultState{MainPath: "/vaults/main", WorkPath: "/vaults/work", Default: "main"},
			wantChecks: []string{
				`"main_vault": "/vaults/main"`,
				`"work_vault": "/vaults/work"`,
				`"default_vault": "main"`,
			},
		},
		{
			name:  "existingcfg",
			seed:  "{\n  \"opencode_base_url\": \"http://127.0.0.1:4199\"\n}\n",
			state: VaultState{MainPath: "/vaults/main", WorkPath: "/vaults/work", Default: "work"},
			wantChecks: []string{
				`"main_vault": "/vaults/main"`,
				`"work_vault": "/vaults/work"`,
				`"default_vault": "work"`,
				`"opencode_base_url": "http://127.0.0.1:4199"`,
			},
		},
		{
			name:      "rejects malformed existing json",
			seed:      "{\n",
			state:     VaultState{MainPath: "/vaults/main"},
			wantErr:   true,
			wantExact: "{\n",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			homeDir := t.TempDir()
			configJSON := filepath.Join(homeDir, ".config", "pde", "config.json")
			if tt.seed != "" {
				if err := os.MkdirAll(filepath.Dir(configJSON), 0o755); err != nil {
					t.Fatalf("mkdir config parent: %v", err)
				}
				mustWriteFile(t, configJSON, tt.seed, 0o644)
			}

			err := writeVaultState(homeDir, tt.state)
			if tt.wantErr {
				if err == nil {
					t.Fatal("expected write vault state to fail")
				}
				if content := mustFileContents(t, configJSON, ""); content != tt.wantExact {
					t.Fatalf("expected config to remain unchanged, got:\n%s", content)
				}
				return
			}
			if err != nil {
				t.Fatalf("write vault state: %v", err)
			}

			content := mustFileContents(t, configJSON, "")
			for _, want := range tt.wantChecks {
				if !strings.Contains(content, want) {
					t.Fatalf("expected %q in output, got:\n%s", want, content)
				}
			}
		})
	}
}
