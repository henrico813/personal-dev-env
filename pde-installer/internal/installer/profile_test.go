package installer

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/profile"
)

func TestFreshProfilesUseDefaults(t *testing.T) {
	tests := map[string]struct {
		mode profileMode
		want profile.Profile
	}{
		"install": {mode: installProfile, want: profile.Full},
		"list":    {mode: readProfile, want: profile.Full},
	}
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			got, err := resolveProfile(t.TempDir(), "", test.mode)
			if err != nil || got != test.want {
				t.Fatalf("resolveProfile() = %q, %v", got, err)
			}
		})
	}
}

func TestExplicitProfileIsSelected(t *testing.T) {
	got, err := resolveProfile(t.TempDir(), "terminal", installProfile)
	if err != nil || got != profile.Terminal {
		t.Fatalf("resolveProfile() = %q, %v", got, err)
	}
}

func TestSavedProfileIsLoaded(t *testing.T) {
	home := profileHome(t, `{"profile":"terminal"}`)
	got, err := resolveProfile(home, "", requireProfile)
	if err != nil || got != profile.Terminal {
		t.Fatalf("resolveProfile() = %q, %v", got, err)
	}
}

func TestMissingProfileBlocksUpdate(t *testing.T) {
	_, err := resolveProfile(t.TempDir(), "", requireProfile)
	if err == nil || !strings.Contains(err.Error(), "no saved profile") {
		t.Fatalf("resolveProfile() error = %v", err)
	}
}

func TestLegacyProfilesRequireRepair(t *testing.T) {
	tests := map[string]func(*testing.T) string{
		"config without profile": func(t *testing.T) string { return profileHome(t, `{}`) },
		"legacy paths file": func(t *testing.T) string {
			home := t.TempDir()
			writeProfileFile(t, filepath.Join(home, ".config", "pde", "paths.env"), "PDE_MAIN_VAULT=/vault\n")
			return home
		},
	}
	for name, setup := range tests {
		t.Run(name, func(t *testing.T) {
			_, err := resolveProfile(setup(t), "", readProfile)
			if err == nil || !strings.Contains(err.Error(), "has no profile") {
				t.Fatalf("resolveProfile() error = %v", err)
			}
		})
	}
}

func TestInvalidSavedProfileIsRejected(t *testing.T) {
	home := profileHome(t, `{"profile":"small"}`)
	_, err := resolveProfile(home, "", readProfile)
	if err == nil || !strings.Contains(err.Error(), `invalid profile "small"`) {
		t.Fatalf("resolveProfile() error = %v", err)
	}
}

func TestProfileTransitions(t *testing.T) {
	tests := map[string]struct {
		saved, request string
		want           profile.Profile
		wantError      string
	}{
		"same profile":     {saved: "full", request: "full", want: profile.Full},
		"terminal expands": {saved: "terminal", request: "full", want: profile.Full},
		"full downgrade":   {saved: "full", request: "terminal", wantError: "cannot change profile from full to terminal"},
	}
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			home := profileHome(t, `{"profile":"`+test.saved+`"}`)
			got, err := resolveProfile(home, test.request, installProfile)
			if test.wantError != "" {
				if err == nil || !strings.Contains(err.Error(), test.wantError) {
					t.Fatalf("resolveProfile() error = %v", err)
				}
				return
			}
			if err != nil || got != test.want {
				t.Fatalf("resolveProfile() = %q, %v", got, err)
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
