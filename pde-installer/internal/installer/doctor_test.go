package installer

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/colorprofile"
	"pde-installer/internal/profile"
	"pde-installer/internal/run"
)

// Archive tools are required even without the full build toolchain.
func TestDoctorRequiresTerminalTools(t *testing.T) {
	t.Setenv("PATH", terminalProbeBin(t, "tar"))
	cfg := terminalTestConfig(t)
	writeInvalidFullMetadata(t, cfg)
	var stderr bytes.Buffer
	err := hostPreflight(cfg, run.Runner{Stdout: &bytes.Buffer{}, Stderr: &stderr}, preflightQuiet)
	if err == nil || !strings.Contains(err.Error(), "doctor found 1 problem(s)") {
		t.Fatalf("hostPreflight() error = %v", err)
	}
	if !strings.Contains(stderr.String(), "tar: install host tar") {
		t.Fatalf("doctor diagnostic = %q", stderr.String())
	}
}

// Terminal installs deliberately omit compilers and full-only metadata.
func TestDoctorSkipsBuildTools(t *testing.T) {
	t.Setenv("PATH", terminalProbeBin(t, "compiler"))
	cfg := terminalTestConfig(t)
	writeInvalidFullMetadata(t, cfg)
	if err := hostPreflight(cfg, run.Runner{Stdout: &bytes.Buffer{}, Stderr: &bytes.Buffer{}}, preflightQuiet); err != nil {
		t.Fatalf("hostPreflight() error = %v", err)
	}
}

func TestDoctorRejectsInvalidProfile(t *testing.T) {
	cfg := terminalTestConfig(t)
	cfg.Profile = profile.Profile("desktop")
	err := hostPreflight(cfg, run.Runner{}, preflightQuiet)
	if err == nil || !strings.Contains(err.Error(), `invalid profile "desktop"`) {
		t.Fatalf("hostPreflight() error = %v", err)
	}
}

type copilotReportCase struct {
	name        string
	profile     profile.Profile
	mode        preflightMode
	createFiles bool
	wantCopilot string
}

func TestDoctorCopilotReportModes(t *testing.T) {
	for _, tt := range copilotReportCases() {
		t.Run(tt.name, func(t *testing.T) {
			cfg, xdgConfig := newDoctorCopilotFixture(t, tt.profile)
			if tt.createFiles {
				createCopilotStatusFiles(t, cfg.Home, xdgConfig)
			}

			output := runDoctorPreflight(t, cfg, tt.mode)
			assertCopilotOutput(t, output, tt.wantCopilot)
		})
	}
}

func copilotReportCases() []copilotReportCase {
	return []copilotReportCase{
		{
			name:        "full report finds installed files",
			profile:     profile.Full,
			mode:        preflightReport,
			createFiles: true,
			wantCopilot: "status  GitHub Copilot plugin: installed\n" +
				"status  GitHub Copilot credentials: present\n",
		},
		{
			name:    "full report warns about missing files",
			profile: profile.Full,
			mode:    preflightReport,
			wantCopilot: "warning GitHub Copilot plugin: missing; run pde-installer install\n" +
				"warning GitHub Copilot credentials: missing; run :Copilot auth in Neovim\n",
		},
		{
			name:    "full quiet mode omits Copilot",
			profile: profile.Full,
			mode:    preflightQuiet,
		},
		{
			name:    "terminal report omits Copilot",
			profile: profile.Terminal,
			mode:    preflightReport,
		},
	}
}

func newDoctorCopilotFixture(t *testing.T, selected profile.Profile) (config, string) {
	t.Helper()
	t.Setenv("PATH", terminalProbeBin(t, ""))

	cwd, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	repoRoot, ok := repository(filepath.Join(cwd, "..", "..", ".."))
	if !ok {
		t.Fatalf("repository root not found from %q", cwd)
	}

	home := t.TempDir()
	xdgConfig := t.TempDir()
	t.Setenv("XDG_CONFIG_HOME", xdgConfig)
	return config{
		Home:         home,
		RepoRoot:     repoRoot,
		LocalBin:     filepath.Join(home, ".local", "bin"),
		AquaRoot:     filepath.Join(home, ".local", "share", "aquaproj-aqua"),
		Profile:      selected,
		ColorProfile: colorprofile.TokyoNight,
	}, xdgConfig
}

func createCopilotStatusFiles(t *testing.T, home, xdgConfig string) {
	t.Helper()
	for _, path := range []string{
		filepath.Join(home, ".config", "nvim", "pack", "plugins", "start", "copilot.lua", "lua", "copilot", "init.lua"),
		filepath.Join(xdgConfig, "github-copilot", "auth.db"),
	} {
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, nil, 0o600); err != nil {
			t.Fatal(err)
		}
	}
}

func runDoctorPreflight(t *testing.T, cfg config, mode preflightMode) string {
	t.Helper()
	var stdout, stderr bytes.Buffer
	if err := hostPreflight(cfg, run.Runner{DryRun: true, Stdout: &stdout, Stderr: &stderr}, mode); err != nil {
		t.Fatalf("hostPreflight() error = %v; stderr = %q", err, stderr.String())
	}
	return stdout.String()
}

func assertCopilotOutput(t *testing.T, output, want string) {
	t.Helper()
	if want == "" {
		if strings.Contains(output, "GitHub Copilot") {
			t.Fatalf("unexpected Copilot output: %q", output)
		}
		return
	}
	if !strings.Contains(output, want) {
		t.Fatalf("Copilot output missing %q from %q", want, output)
	}
}

type copilotStatusCase struct {
	name        string
	createFiles bool
	want        string
}

func TestCopilotStatusOutput(t *testing.T) {
	for _, tt := range copilotStatusCases() {
		t.Run(tt.name, func(t *testing.T) {
			home, xdgConfig := newCopilotStatusFixture(t)
			if tt.createFiles {
				createCopilotStatusFiles(t, home, xdgConfig)
			}

			output := runCopilotStatus(t, home)
			assertExactCopilotOutput(t, output, tt.want)
		})
	}
}

func copilotStatusCases() []copilotStatusCase {
	return []copilotStatusCase{
		{
			name:        "reports installed files",
			createFiles: true,
			want: "status  GitHub Copilot plugin: installed\n" +
				"status  GitHub Copilot credentials: present\n",
		},
		{
			name: "warns about missing files",
			want: "warning GitHub Copilot plugin: missing; run pde-installer install\n" +
				"warning GitHub Copilot credentials: missing; run :Copilot auth in Neovim\n",
		},
	}
}

func newCopilotStatusFixture(t *testing.T) (string, string) {
	t.Helper()
	home := t.TempDir()
	xdgConfig := t.TempDir()
	t.Setenv("XDG_CONFIG_HOME", xdgConfig)
	return home, xdgConfig
}

func runCopilotStatus(t *testing.T, home string) string {
	t.Helper()
	var output bytes.Buffer
	if err := reportCopilotStatus(home, &output); err != nil {
		t.Fatalf("reportCopilotStatus() error = %v", err)
	}
	return output.String()
}

func assertExactCopilotOutput(t *testing.T, output, want string) {
	t.Helper()
	if output != want {
		t.Fatalf("Copilot status = %q, want %q", output, want)
	}
}
