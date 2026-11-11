package homebrew

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/profile"
	"pde-installer/internal/run"
	"pde-installer/internal/testfile"
)

// A rerun of the install must not reinstall formulae the Mac already has,
// because brew install upgrades them and their dependents.
func TestReconcileInstallsOnlyMissingFormulae(t *testing.T) {
	log := installFakeBrew(t, "git\ntmux\n")
	manager := New("client", "", run.Runner{Stdout: &strings.Builder{}, Stderr: &strings.Builder{}})
	manager.goarch = "arm64"

	if err := manager.Reconcile(); err != nil {
		t.Fatal(err)
	}

	got, err := os.ReadFile(log)
	if err != nil {
		t.Fatal(err)
	}
	if string(got) != "install bat eza fd python@3.13\n" {
		t.Fatalf("brew calls = %q, want one install of the missing formulae", got)
	}
}

// Validate refuses an Intel Mac, full and a desktop with a message that names
// a selection that works, and accepts client on Apple Silicon.
func TestValidateAllowsOnlySupportedMacSetups(t *testing.T) {
	tests := []struct {
		name    string
		profile profile.Profile
		desktop profile.Desktop
		goarch  string
		wantErr string
	}{
		{
			name:    "full profile",
			profile: "full",
			goarch:  "arm64",
			wantErr: "the full profile does not install on macOS; use terminal or client",
		},
		{
			name:    "desktop",
			profile: "client",
			desktop: "i3",
			goarch:  "arm64",
			wantErr: "the i3 desktop needs Linux; macOS installs terminal or client without one",
		},
		{
			name:    "Intel Mac",
			profile: "terminal",
			goarch:  "amd64",
			wantErr: "macOS on amd64 is not supported; pdev needs an Apple Silicon Mac",
		},
		{
			name:    "client on Apple Silicon",
			profile: "client",
			goarch:  "arm64",
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			installFakeBrew(t, "")
			manager := New(test.profile, test.desktop, run.Runner{})
			manager.goarch = test.goarch

			err := manager.Validate()

			got := ""
			if err != nil {
				got = err.Error()
			}
			if got != test.wantErr {
				t.Fatalf("Validate() error = %q, want %q", got, test.wantErr)
			}
		})
	}
}

// A brew on PATH that prints installed for list and logs every other call.
func installFakeBrew(t *testing.T, installed string) string {
	t.Helper()
	bin := t.TempDir()
	log := filepath.Join(bin, "brew.log")
	script := "#!/bin/sh\nif [ \"$1\" = list ]; then printf '%s' '" + installed + "'; exit 0; fi\nprintf '%s\\n' \"$*\" >> '" + log + "'\n"
	testfile.WriteExecutable(t, filepath.Join(bin, "brew"), script)
	t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))
	return log
}
