package installer

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/profile"
	"pde-installer/internal/run"
)

func TestTerminalDoctorProfiles(t *testing.T) {
	for _, test := range []struct {
		name    string
		missing string
		wantErr bool
	}{
		{name: "missing tar", missing: "tar", wantErr: true},
		{name: "no compiler", missing: "compiler"},
	} {
		t.Run(test.name, func(t *testing.T) {
			bin := terminalProbeBin(t, test.missing)
			t.Setenv("PATH", bin)
			cfg := terminalTestConfig(t)
			runner := run.Runner{Stdout: &bytes.Buffer{}, Stderr: &bytes.Buffer{}}
			err := hostPreflight(cfg, runner, preflightQuiet)
			if (err != nil) != test.wantErr {
				t.Fatalf("hostPreflight() error = %v, wantErr %t", err, test.wantErr)
			}
		})
	}
}

func TestTerminalListIgnoresFullMetadata(t *testing.T) {
	t.Setenv("PATH", terminalProbeBin(t, ""))
	cfg := terminalTestConfig(t)
	if err := os.MkdirAll(filepath.Join(cfg.Home, ".local", "share", "pde", "npm"), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(cfg.Home, ".local", "share", "pde", "npm", "package.json"), []byte("invalid"), 0o644); err != nil {
		t.Fatal(err)
	}
	var output bytes.Buffer
	if err := list(cfg, run.Runner{Stdout: &output, Stderr: &bytes.Buffer{}}); err != nil {
		t.Fatalf("list() error = %v", err)
	}
	if !strings.Contains(output.String(), "ya") {
		t.Fatalf("list output omits ya: %s", output.String())
	}
	if strings.Contains(output.String(), "opencode-ai") || strings.Contains(output.String(), "neovim") {
		t.Fatalf("list output includes full-only item: %s", output.String())
	}
}

func terminalTestConfig(t *testing.T) config {
	t.Helper()
	home := t.TempDir()
	return config{Home: home, RepoRoot: testRepositoryRoot(t), LocalBin: filepath.Join(home, ".local", "bin"), AquaRoot: filepath.Join(home, ".local", "share", "aquaproj-aqua"), Profile: profile.Terminal}
}

func terminalProbeBin(t *testing.T, missing string) string {
	t.Helper()
	bin := t.TempDir()
	for _, name := range []string{"apt-get", "dpkg-query", "sudo", "sh", "tar", "gzip", "xz", "unzip", "sed", "awk", "grep", "file", "curl", "cc", "gcc", "clang", "c++", "g++", "clang++"} {
		if name == missing || missing == "compiler" && (name == "cc" || name == "gcc" || name == "clang" || name == "c++" || name == "g++" || name == "clang++") {
			continue
		}
		contents := "#!/bin/sh\n"
		if name == "dpkg-query" {
			contents += "printf 'ii\\t1\\n'\n"
		} else {
			contents += "exit 0\n"
		}
		if err := os.WriteFile(filepath.Join(bin, name), []byte(contents), 0o755); err != nil {
			t.Fatal(err)
		}
	}
	return bin
}
