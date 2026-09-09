package installer

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/profile"
)

func TestProfileResolution(t *testing.T) {
	tests := map[string]struct {
		config, request, failure string
		mode                     profileMode
		want                     profile.Profile
		legacy                   bool
	}{
		"fresh install":   {mode: installProfile, want: profile.Full},
		"fresh terminal":  {request: "terminal", mode: installProfile, want: profile.Terminal},
		"saved terminal":  {config: `{"profile":"terminal"}`, mode: requireProfile, want: profile.Terminal},
		"fresh update":     {mode: requireProfile, failure: "no saved profile"},
		"fresh list":       {mode: readProfile, want: profile.Full},
		"legacy config":    {config: `{}`, mode: installProfile, failure: "has no profile"},
		"legacy paths":     {legacy: true, mode: readProfile, failure: "has no profile"},
		"invalid saved":    {config: `{"profile":"small"}`, mode: readProfile, failure: "invalid profile"},
	}
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			home := t.TempDir()
			directory := filepath.Join(home, ".config", "pde")
			if test.config != "" || test.legacy {
				if err := os.MkdirAll(directory, 0o755); err != nil { t.Fatal(err) }
			}
			if test.config != "" {
				if err := os.WriteFile(filepath.Join(directory, "config.json"), []byte(test.config), 0o644); err != nil { t.Fatal(err) }
			}
			if test.legacy {
				if err := os.WriteFile(filepath.Join(directory, "paths.env"), []byte("PDE_MAIN_VAULT=/vault\n"), 0o644); err != nil { t.Fatal(err) }
			}
			got, err := resolveProfile(home, test.request, test.mode)
			if test.failure != "" {
				if err == nil || !strings.Contains(err.Error(), test.failure) { t.Fatalf("resolveProfile() error = %v", err) }
				return
			}
			if err != nil || got != test.want { t.Fatalf("resolveProfile() = %q, %v", got, err) }
		})
	}
}

func TestProfileTransitions(t *testing.T) {
	for name, test := range map[string]struct { saved, request string; want profile.Profile; fail bool }{
		"same profile":          {saved: "full", request: "full", want: profile.Full},
		"terminal expands":      {saved: "terminal", request: "full", want: profile.Full},
		"full never downgrades": {saved: "full", request: "terminal", fail: true},
	} {
		t.Run(name, func(t *testing.T) {
			home := t.TempDir()
			path := filepath.Join(home, ".config", "pde", "config.json")
			if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil { t.Fatal(err) }
			if err := os.WriteFile(path, []byte(`{"profile":"`+test.saved+`"}`), 0o644); err != nil { t.Fatal(err) }
			got, err := resolveProfile(home, test.request, installProfile)
			if (err != nil) != test.fail || got != test.want { t.Fatalf("resolveProfile() = %q, %v", got, err) }
		})
	}
}
