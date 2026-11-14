package installer

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	"pde-installer/internal/profile"
)

func TestFreshReadsUseFull(t *testing.T) {
	got, err := resolveProfile(t.TempDir(), "", readProfile)
	if err != nil || got != "full" {
		t.Fatalf("resolveProfile() = %q, %v", got, err)
	}
}

func TestFreshInstallRequiresSelection(t *testing.T) {
	_, err := resolveProfile(t.TempDir(), "", installProfile)
	if err == nil || !strings.Contains(err.Error(), "no saved profile") {
		t.Fatalf("resolveProfile() error = %v", err)
	}
}

func TestExplicitProfileIsSelected(t *testing.T) {
	got, err := resolveProfile(t.TempDir(), "terminal", installProfile)
	if err != nil || got != "terminal" {
		t.Fatalf("resolveProfile() = %q, %v", got, err)
	}
}

func TestSavedProfileIsLoaded(t *testing.T) {
	home := profileHome(t, `{"profile":"terminal"}`)
	got, err := resolveProfile(home, "", installProfile)
	if err != nil || got != "terminal" {
		t.Fatalf("resolveProfile() = %q, %v", got, err)
	}
}

func TestMissingProfileBlocksInstall(t *testing.T) {
	_, err := resolveProfile(t.TempDir(), "", installProfile)
	if err == nil || !strings.Contains(err.Error(), "install terminal") {
		t.Fatalf("resolveProfile() error = %v", err)
	}
}

func TestLegacyInstallsUseFull(t *testing.T) {
	tests := map[string]func(*testing.T) string{
		"config with install path": func(t *testing.T) string { return profileHome(t, `{"install_path":"/repo"}`) },
		"legacy paths file": func(t *testing.T) string {
			home := t.TempDir()
			writeProfileFile(t, filepath.Join(home, ".config", "pde", "paths.env"), "PDE_MAIN_VAULT=/vault\n")
			return home
		},
	}
	for name, setup := range tests {
		t.Run(name, func(t *testing.T) {
			got, err := resolveProfile(setup(t), "", installProfile)
			if err != nil || got != "full" {
				t.Fatalf("resolveProfile() = %q, %v", got, err)
			}
		})
	}
}

func TestProfilelessConfigRequiresRepair(t *testing.T) {
	_, err := resolveProfile(profileHome(t, `{}`), "", readProfile)
	if err == nil || !strings.Contains(err.Error(), "has no profile") {
		t.Fatalf("resolveProfile() error = %v", err)
	}
}

func TestInvalidSavedProfileIsRejected(t *testing.T) {
	home := profileHome(t, `{"profile":"small"}`)
	_, err := resolveProfile(home, "", readProfile)
	if err == nil || !strings.Contains(err.Error(), `invalid profile "small"`) {
		t.Fatalf("resolveProfile() error = %v", err)
	}
}

func TestExplicitSelectionsOverrideSaved(t *testing.T) {
	tests := map[string]struct {
		saved, request string
		want           profile.Profile
	}{
		"same profile":     {saved: "full", request: "full", want: "full"},
		"terminal expands": {saved: "terminal", request: "full", want: "full"},
		"full narrows":     {saved: "full", request: "terminal", want: "terminal"},
	}
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			home := profileHome(t, `{"profile":"`+test.saved+`"}`)
			got, err := resolveProfile(home, test.request, installProfile)
			if err != nil || got != test.want {
				t.Fatalf("resolveProfile() = %q, %v", got, err)
			}
		})
	}
}

// Installing a profile keeps the saved desktop, and installing a desktop keeps
// the saved profile, so neither argument resets the other.
func TestInstallArgumentKeepsOtherSavedSelection(t *testing.T) {
	tests := map[string]struct {
		saved       string
		args        []string
		wantProfile profile.Profile
		wantDesktop profile.Desktop
	}{
		"profile keeps desktop": {
			saved: `{"profile":"full","desktop":"i3"}`, args: []string{"client"},
			wantProfile: "client", wantDesktop: "i3",
		},
		"desktop keeps profile": {
			saved: `{"profile":"terminal"}`, args: []string{"sway"},
			wantProfile: "terminal", wantDesktop: "sway",
		},
		"desktop replaces desktop": {
			saved: `{"profile":"full","desktop":"i3"}`, args: []string{"sway"},
			wantProfile: "full", wantDesktop: "sway",
		},
		"bare install keeps both": {
			saved:       `{"profile":"client","desktop":"sway"}`,
			wantProfile: "client", wantDesktop: "sway",
		},
		"no desktop saved": {
			saved: `{"profile":"full"}`, args: []string{"full"},
			wantProfile: "full", wantDesktop: "",
		},
	}
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			home := profileHome(t, test.saved)

			got, err := resolveInstallSelections(config{Home: home}, test.args, "")

			if err != nil {
				t.Fatal(err)
			}
			if got.Profile != test.wantProfile || got.Desktop != test.wantDesktop {
				t.Fatalf("profile, desktop = %q, %q; want %q, %q", got.Profile, got.Desktop, test.wantProfile, test.wantDesktop)
			}
		})
	}
}

// Install writes the interval into the update timer. A typo must stop the
// install, or the owner's machine silently keeps the hourly default.
func TestResolveInstallSelectionsReadsUpdateInterval(t *testing.T) {
	tests := []struct {
		name      string
		saved     string
		want      time.Duration
		wantError string
	}{
		{
			name:  "no key",
			saved: `{"profile":"full"}`,
			want:  time.Hour,
		},
		{
			name:  "set",
			saved: `{"profile":"full","update_interval":"90m"}`,
			want:  90 * time.Minute,
		},
		{
			name:      "under a minute",
			saved:     `{"profile":"full","update_interval":"30s"}`,
			wantError: `not "30s"`,
		},
		{
			name:      "typo",
			saved:     `{"profile":"full","update_interval":"6 hours"}`,
			wantError: `not "6 hours"`,
		},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			home := profileHome(
				t,
				test.saved,
			)

			got, err := resolveInstallSelections(
				config{Home: home},
				nil,
				"",
			)

			if test.wantError != "" {
				if err == nil || !strings.Contains(
					err.Error(),
					test.wantError,
				) {
					t.Fatalf(
						"error = %v, want one containing %s",
						err,
						test.wantError,
					)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			if got.UpdateInterval != test.want {
				t.Fatalf(
					"UpdateInterval = %v, want %v",
					got.UpdateInterval,
					test.want,
				)
			}
		})
	}
}

func profileHome(t *testing.T, config string) string {
	t.Helper()
	home := t.TempDir()
	writeProfileFile(t, filepath.Join(home, ".config", "pde", "config.json"), config)
	return home
}

func writeProfileFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}
