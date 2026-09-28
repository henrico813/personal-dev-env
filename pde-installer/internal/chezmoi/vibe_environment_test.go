package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"testing"
)

const vibeEnvironmentTemplate = `# Fill in the Goog endpoint and key before sourcing this file.
export GOOG_BASE_URL='https://models.example/v1'
export GOOG_API_KEY=''
export GOOG_MODEL='qwen3.8'
`

func TestVibeEnvironmentCreatesPrivateTemplate(t *testing.T) {
	home := t.TempDir()
	runVibeEnvironmentTemplate(t, home, "full")

	envDir := filepath.Join(home, ".config", "vibe")
	envFile := filepath.Join(envDir, "goog.env")
	if got := readVibeEnvironmentFile(t, envFile); got != vibeEnvironmentTemplate {
		t.Fatalf("template = %q", got)
	}
	assertVibeEnvironmentMode(t, envDir, 0o700)
	assertVibeEnvironmentMode(t, envFile, 0o600)
}

func TestVibeEnvironmentPreservesExistingFile(t *testing.T) {
	home := t.TempDir()
	envFile := filepath.Join(home, ".config", "vibe", "goog.env")
	writeApplyFile(t, envFile, "export GOOG_API_KEY='configured'\n")

	runVibeEnvironmentTemplate(t, home, "full")

	if got := readVibeEnvironmentFile(t, envFile); got != "export GOOG_API_KEY='configured'\n" {
		t.Fatalf("existing environment = %q", got)
	}
}

func TestVibeEnvironmentSkipsTerminalProfile(t *testing.T) {
	home := t.TempDir()
	runVibeEnvironmentTemplate(t, home, "terminal")

	path := filepath.Join(home, ".config", "vibe", "goog.env")
	if _, err := os.Stat(path); !os.IsNotExist(err) {
		t.Fatalf("terminal environment file = %v, want absent", err)
	}
}

func TestVibeEnvironmentRejectsSymlinkParents(t *testing.T) {
	for _, parent := range []string{".config", filepath.Join(".config", "vibe")} {
		t.Run(parent, func(t *testing.T) {
			home := t.TempDir()
			target := t.TempDir()
			path := filepath.Join(home, parent)
			if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
				t.Fatal(err)
			}
			if err := os.Symlink(target, path); err != nil {
				t.Fatal(err)
			}

			if _, err := runVibeEnvironment(t, home, "full", nil); err == nil {
				t.Fatal("symlinked parent unexpectedly accepted")
			}
		})
	}
}

func TestVibeEnvironmentRejectsSymlinkFile(t *testing.T) {
	home := t.TempDir()
	envDir := filepath.Join(home, ".config", "vibe")
	if err := os.MkdirAll(envDir, 0o700); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(home, "configured.env")
	if err := os.WriteFile(target, []byte("export GOOG_API_KEY='configured'\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(target, filepath.Join(envDir, "goog.env")); err != nil {
		t.Fatal(err)
	}

	if _, err := runVibeEnvironment(t, home, "full", nil); err == nil {
		t.Fatal("symlinked environment unexpectedly accepted")
	}
}

func TestVibeEnvironmentRejectsRacedSymlink(t *testing.T) {
	home := t.TempDir()
	envDir := filepath.Join(home, ".config", "vibe")
	if err := os.MkdirAll(envDir, 0o700); err != nil {
		t.Fatal(err)
	}
	target := filepath.Join(home, "configured.env")
	if err := os.WriteFile(target, []byte("configured\n"), 0o600); err != nil {
		t.Fatal(err)
	}
	realLn, err := exec.LookPath("ln")
	if err != nil {
		t.Fatal(err)
	}
	fakeBin := t.TempDir()
	writeExecutable(t, filepath.Join(fakeBin, "ln"), `#!/bin/sh
"$PDE_TEST_REAL_LN" -s "$PDE_TEST_SYMLINK_TARGET" "$2"
exit 1
`)

	_, err = runVibeEnvironment(t, home, "full", []string{
		"PATH=" + fakeBin + ":" + os.Getenv("PATH"),
		"PDE_TEST_REAL_LN=" + realLn,
		"PDE_TEST_SYMLINK_TARGET=" + target,
	})

	if err == nil {
		t.Fatal("raced symlink unexpectedly accepted")
	}
	info, err := os.Lstat(filepath.Join(envDir, "goog.env"))
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("raced path mode = %v, want symlink", info.Mode())
	}
}

func TestVibeEnvironmentCleansFailedTemporaryWrite(t *testing.T) {
	home := t.TempDir()
	envDir := filepath.Join(home, ".config", "vibe")
	if err := os.MkdirAll(envDir, 0o700); err != nil {
		t.Fatal(err)
	}
	fakeBin := t.TempDir()
	fakeMktemp := filepath.Join(fakeBin, "mktemp")
	leftover := filepath.Join(envDir, ".goog.env.failed")
	fakeCat := filepath.Join(fakeBin, "cat")
	writeExecutable(t, fakeMktemp, "#!/bin/sh\nprintf '%s\\n' \""+leftover+"\"\n")
	writeExecutable(t, fakeCat, "#!/bin/sh\nexit 1\n")

	if _, err := runVibeEnvironment(t, home, "full", []string{"PATH=" + fakeBin + ":" + os.Getenv("PATH")}); err == nil {
		t.Fatal("failed temporary write unexpectedly succeeded")
	}
	if _, err := os.Stat(leftover); !os.IsNotExist(err) {
		t.Fatalf("temporary file = %v, want absent", err)
	}
}

func TestVibeEnvironmentPublishesOneConcurrentFile(t *testing.T) {
	home := t.TempDir()
	contents := renderProfileTemplate(t, "run_after_create_vibe_environment.sh.tmpl", "full")
	script := filepath.Join(t.TempDir(), "create-vibe-environment.sh")
	if err := os.WriteFile(script, []byte(contents), 0o700); err != nil {
		t.Fatal(err)
	}

	commands := []*exec.Cmd{
		exec.Command("sh", script),
		exec.Command("sh", script),
	}
	for _, command := range commands {
		command.Env = append(os.Environ(), "HOME="+home, "PDE_PROFILE=full")
		if err := command.Start(); err != nil {
			t.Fatal(err)
		}
	}
	for _, command := range commands {
		if err := command.Wait(); err != nil {
			t.Fatalf("concurrent publication: %v", err)
		}
	}

	if got := readVibeEnvironmentFile(t, filepath.Join(home, ".config", "vibe", "goog.env")); got != vibeEnvironmentTemplate {
		t.Fatalf("published environment = %q", got)
	}
}

func runVibeEnvironmentTemplate(t *testing.T, home, profile string) {
	t.Helper()
	if _, err := runVibeEnvironment(t, home, profile, nil); err != nil {
		t.Fatalf("run environment template: %v", err)
	}
}

func runVibeEnvironment(t *testing.T, home, profile string, environment []string) ([]byte, error) {
	t.Helper()
	script := filepath.Join(t.TempDir(), "create-vibe-environment.sh")
	contents := renderProfileTemplate(t, "run_after_create_vibe_environment.sh.tmpl", profile)
	if err := os.WriteFile(script, []byte(contents), 0o700); err != nil {
		t.Fatal(err)
	}
	command := exec.Command("sh", script)
	command.Env = append(os.Environ(), "HOME="+home, "PDE_PROFILE="+profile)
	command.Env = append(command.Env, environment...)
	return command.CombinedOutput()
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
