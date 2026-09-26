package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

const vibeEnvironmentTemplate = `# Fill in the compatible endpoint and key before sourcing this file.
export OPENAI_COMPATIBLE_BASE_URL='https://models.example/v1'
export OPENAI_COMPATIBLE_API_KEY=''
export OPENAI_COMPATIBLE_MODEL='example-model'
`

func TestVibeEnvironmentCreatesPrivateTemplate(t *testing.T) {
	home := t.TempDir()
	runVibeEnvironmentTemplate(t, home, "full")

	envDir := filepath.Join(home, ".config", "vibe")
	envFile := filepath.Join(envDir, "openai-compatible.env")
	if got := readVibeEnvironmentFile(t, envFile); got != vibeEnvironmentTemplate {
		t.Fatalf("template = %q", got)
	}
	assertVibeEnvironmentMode(t, envDir, 0o700)
	assertVibeEnvironmentMode(t, envFile, 0o600)
}

func TestVibeEnvironmentPreservesExistingFile(t *testing.T) {
	home := t.TempDir()
	envFile := filepath.Join(home, ".config", "vibe", "openai-compatible.env")
	writeApplyFile(t, envFile, "export OPENAI_COMPATIBLE_API_KEY='configured'\n")

	runVibeEnvironmentTemplate(t, home, "full")

	if got := readVibeEnvironmentFile(t, envFile); got != "export OPENAI_COMPATIBLE_API_KEY='configured'\n" {
		t.Fatalf("existing environment = %q", got)
	}
}

func TestVibeEnvironmentSkipsTerminalProfile(t *testing.T) {
	home := t.TempDir()
	runVibeEnvironmentTemplate(t, home, "terminal")

	path := filepath.Join(home, ".config", "vibe", "openai-compatible.env")
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("terminal environment file = %v, want absent", err)
	}
}

func runVibeEnvironmentTemplate(t *testing.T, home, profile string) {
	t.Helper()
	script := filepath.Join(t.TempDir(), "create-vibe-environment.sh")
	contents := renderProfileTemplate(t, "run_after_create_vibe_environment.sh.tmpl", profile)
	if err := os.WriteFile(script, []byte(contents), 0o700); err != nil {
		t.Fatal(err)
	}

	command := exec.Command("sh", script)
	command.Env = append(os.Environ(), "HOME="+home, "PDE_PROFILE="+profile)
	output, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("run environment template: %v\n%s", err, output)
	}
}

func readVibeEnvironmentFile(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func assertVibeEnvironmentMode(t *testing.T, path string, want os.FileMode) {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != want {
		t.Fatalf("%s mode = %o, want %o", path, got, want)
	}
}
