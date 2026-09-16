package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestOCWPasswordCreatesAndReplacesCredentials(t *testing.T) {
	home := t.TempDir()
	writePasswordRuntime(t, home)

	if output, err := runPasswordCommand(home, "first-pass\nfirst-pass\n"); err != nil {
		t.Fatalf("ocw-password: %v\n%s", err, output)
	}
	path := filepath.Join(home, ".config", "opencode", "server.env")
	if got := readPasswordFile(t, path); got != "OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_PASSWORD=first-pass\n" {
		t.Fatalf("created credentials = %q", got)
	}
	assertPasswordMode(t, filepath.Join(home, ".config", "opencode"), 0o700)
	assertPasswordMode(t, path, 0o600)
	if got := readPasswordFile(t, filepath.Join(home, "systemctl-arguments")); got != "--user daemon-reload\n--user enable opencode-web.service opencode-web-health.timer\n--user restart opencode-web.service\n--user start opencode-web-health.timer\n" {
		t.Fatalf("systemctl arguments = %q", got)
	}

	output, err := runPasswordCommand(home, "second-pass\nsecond-pass\n")
	if err != nil {
		t.Fatalf("ocw-password replacement: %v\n%s", err, output)
	}
	if strings.Contains(output, "second-pass") {
		t.Fatalf("password appeared in output: %q", output)
	}
	if got := readPasswordFile(t, path); got != "OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_PASSWORD=second-pass\n" {
		t.Fatalf("replaced credentials = %q", got)
	}
}

func TestOCWPasswordPreservesFileOnMismatch(t *testing.T) {
	home := t.TempDir()
	writePasswordRuntime(t, home)
	path := filepath.Join(home, ".config", "opencode", "server.env")
	writeApplyFile(t, path, "OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_PASSWORD=old-pass\n")
	if err := os.Chmod(path, 0o600); err != nil {
		t.Fatal(err)
	}

	output, err := runPasswordCommand(home, "new-pass\nwrong-pass\n")
	if err == nil {
		t.Fatalf("ocw-password unexpectedly succeeded: %s", output)
	}
	if !strings.Contains(output, "must match") {
		t.Fatalf("mismatch output = %q", output)
	}
	if got := readPasswordFile(t, path); got != "OPENCODE_SERVER_USERNAME=opencode\nOPENCODE_SERVER_PASSWORD=old-pass\n" {
		t.Fatalf("credentials after mismatch = %q", got)
	}
	assertPasswordMode(t, path, 0o600)
}

func TestOpenCodeServiceRequiresCredentials(t *testing.T) {
	data, err := os.ReadFile(filepath.Join(repoRoot(t), "chezmoi", "dot_config", "systemd", "user", "opencode-web.service"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), "EnvironmentFile=%h/.config/opencode/server.env") {
		t.Fatalf("service does not require credentials: %q", data)
	}

	full := renderProfileTemplate(t, "run_after_configure_opencode_web.sh.tmpl", "full")
	if !strings.Contains(full, "Run ocw-password, then start opencode-web.service.") {
		t.Fatalf("full setup does not explain bootstrap: %q", full)
	}
	terminal := renderProfileTemplate(t, "run_after_configure_opencode_web.sh.tmpl", "terminal")
	if strings.Contains(terminal, "systemctl --user") {
		t.Fatalf("terminal setup starts services: %q", terminal)
	}
}

func runPasswordCommand(home, input string) (string, error) {
	command := exec.Command("zsh", "-fc", "source \"$HOME/.zshrc\"; ocw-password")
	command.Dir = home
	command.Env = append(os.Environ(), "HOME="+home, "PATH="+filepath.Join(home, ".local", "bin")+":"+os.Getenv("PATH"))
	command.Stdin = strings.NewReader(input)
	output, err := command.CombinedOutput()
	return string(output), err
}

func writePasswordRuntime(t *testing.T, home string) {
	t.Helper()
	writeOpenCodeZshRuntime(t, home)
	writeExecutable(t, filepath.Join(home, ".local", "bin", "systemctl"), "#!/bin/sh\nprintf '%s\\n' \"$*\" >>\"$HOME/systemctl-arguments\"\nexit 0\n")
}

func readPasswordFile(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func assertPasswordMode(t *testing.T, path string, want os.FileMode) {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if got := info.Mode().Perm(); got != want {
		t.Fatalf("%s mode = %o, want %o", path, got, want)
	}
}
