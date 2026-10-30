package installer

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/direct"
	"pde-installer/internal/profile"
	"pde-installer/internal/run"
)

// A machine that once had full keeps Go, Rust, Keychain and Moshi Hook in its
// releases folder. Installing client there would delete them, so the install
// refuses.
func TestClientInstallRefusesToDeleteReleases(t *testing.T) {
	cfg := terminalTestConfig(t)
	cfg.Profile = profile.Client
	statePath := filepath.Join(
		direct.New(cfg.Home, run.Runner{}).ToolsRoot(),
		".pde-state.json",
	)
	if err := os.MkdirAll(filepath.Dir(statePath), 0o755); err != nil {
		t.Fatal(err)
	}
	state := `{"platform":"linux/amd64","tools":{"neovim":{},"go":{},"rust":{},"node":{},"keychain":{},"moshi-hook":{},"herdr":{},"nobs":{}}}`
	if err := os.WriteFile(statePath, []byte(state), 0o644); err != nil {
		t.Fatal(err)
	}

	err := reconcile(cfg, run.Runner{DryRun: true})

	if err == nil || !strings.Contains(err.Error(), "would delete go, rust, keychain, moshi-hook from") {
		t.Fatalf("reconcile() error = %v, want a refusal naming full's release tools", err)
	}
}
