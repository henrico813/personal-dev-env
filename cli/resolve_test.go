package main

import (
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestVaultPaths(t *testing.T) {
	homeDir := t.TempDir()
	t.Setenv("HOME", homeDir)
	mainVault := filepath.Join(homeDir, "main")
	workVault := filepath.Join(homeDir, "work")
	if err := os.MkdirAll(mainVault, 0o755); err != nil {
		t.Fatalf("mkdir main vault: %v", err)
	}
	if err := os.MkdirAll(workVault, 0o755); err != nil {
		t.Fatalf("mkdir work vault: %v", err)
	}

	tests := []struct {
		name     string
		state    VaultState
		selector string
		want     []string
		wantErr  bool
	}{
		{name: "defaultmain", state: VaultState{MainPath: `"$HOME/main"`, WorkPath: `"$HOME/work"`, Default: `"main"`}, selector: "default", want: []string{mainVault}},
		{name: "defaultwork", state: VaultState{MainPath: `"$HOME/main"`, WorkPath: `"~/work"`, Default: `"work"`}, selector: "default", want: []string{workVault}},
		{name: "any", state: VaultState{MainPath: `"$HOME/main"`, WorkPath: `"$HOME/work"`, Default: `"main"`}, selector: "any", want: []string{mainVault, workVault}},
		{name: "invalidpath", state: VaultState{MainPath: `"$HOME/main"`, WorkPath: `"$HOME/missing"`, Default: `"main"`}, selector: "any", wantErr: true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			got, err := selectVaultPaths(tt.state, tt.selector)
			if (err != nil) != tt.wantErr {
				t.Fatalf("select vault paths error = %v wantErr %v", err, tt.wantErr)
			}
			if tt.wantErr {
				return
			}
			if !reflect.DeepEqual(got, tt.want) {
				t.Fatalf("select vault paths = %#v want %#v", got, tt.want)
			}
		})
	}
}
