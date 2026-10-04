package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

const (
	cleanupScriptTemplate = "run_once_before_remove_opencode_web_service.sh.tmpl"
	removeFileTemplate    = ".chezmoiremove"
)

var openCodeWebUnitTargets = []string{
	".config/systemd/user/opencode-web.service",
	".config/systemd/user/opencode-web-health.service",
	".config/systemd/user/opencode-web-health.timer",
}

var openCodeWebUnits = []string{
	"opencode-web.service",
	"opencode-web-health.timer",
	"opencode-web-health.service",
}

// Removing the unit sources does not delete live unit files, so the removal
// list must name them or existing hosts keep running the retired web server.
func TestRemoveFileTargetsWebUnits(t *testing.T) {
	text := renderProfileTemplate(t, removeFileTemplate, "full")

	for _, target := range openCodeWebUnitTargets {
		if !containsLine(text, target) {
			t.Errorf("%s omits removal target %q", removeFileTemplate, target)
		}
	}
	if containsLine(text, ".config/opencode/server.env") {
		t.Errorf("%s removes the operator-owned server.env", removeFileTemplate)
	}
}

// chezmoi skips a removal target that .chezmoiignore matches, so the units must
// stay out of the ignore list or the live service would never be cleaned up.
// Existing hosts keep the old command and skill files unless the installer deletes them, so this test protects cleanup of both paths.
func TestRemoveFileTargetsDocumentCodebase(t *testing.T) {
	targets := []string{
		".config/opencode/commands/document_codebase.md",
		".codex/skills/document-codebase",
	}
	removeText := renderProfileTemplate(t, removeFileTemplate, "full")
	for _, target := range targets {
		if !containsLine(removeText, target) {
			t.Errorf("%s omits removal target %q", removeFileTemplate, target)
		}
	}
	for _, selected := range []string{"full", "terminal"} {
		t.Run(selected, func(t *testing.T) {
			ignoreText := renderProfileTemplate(t, ".chezmoiignore.tmpl", selected)
			for _, target := range targets {
				if containsLine(ignoreText, target) {
					t.Errorf("%s ignores removal target %q", selected, target)
				}
			}
		})
	}
}

func TestIgnoreTemplateKeepsRemoveTargets(t *testing.T) {
	for _, selected := range []string{"full", "terminal"} {
		t.Run(selected, func(t *testing.T) {
			text := renderProfileTemplate(t, ".chezmoiignore.tmpl", selected)
			for _, target := range openCodeWebUnitTargets {
				if containsLine(text, target) {
					t.Errorf("%s ignores removal target %q", selected, target)
				}
			}
		})
	}
}

// The one-time script must disable and stop the live units while their files
// still exist, then reload systemd, or the public service survives the apply.
func TestCleanupScriptDisablesWebUnits(t *testing.T) {
	home := t.TempDir()
	writeExecutable(t, filepath.Join(home, ".local", "bin", "systemctl"), fakeSystemctlScript("0", ""))

	output, err := runCleanupScript(t, home, "full", []string{"PATH=" + filepath.Join(home, ".local", "bin") + ":" + os.Getenv("PATH")})
	if err != nil {
		t.Fatalf("cleanup script failed: %v\n%s", err, output)
	}

	calls := readSystemctlCalls(t, home)
	for _, unit := range openCodeWebUnits {
		assertSystemctlCallCount(t, calls, "--user disable --now "+unit, 1)
		assertSystemctlCallCount(t, calls, "--user reset-failed "+unit, 1)
	}
	assertSystemctlCallCount(t, calls, "--user daemon-reload", 1)
	if strings.Contains(string(output), "WARNING") {
		t.Fatalf("cleanup script warned without unit files:\n%s", output)
	}
}

// A missing unit makes systemctl fail that one disable call; the loop must keep
// going so the remaining enabled units are still stopped, and the symlink that
// enabled the failed unit must still be removed, with a warning.
func TestCleanupScriptContinuesAfterDisableFailure(t *testing.T) {
	home := t.TempDir()
	writeExecutable(t, filepath.Join(home, ".local", "bin", "systemctl"), fakeSystemctlScript("0", "opencode-web-health.timer"))
	writeUserUnitFile(t, home, "opencode-web-health.timer")
	failedLink := writeWantsSymlink(t, home, "timers.target.wants", "opencode-web-health.timer")

	output, err := runCleanupScript(t, home, "full", []string{"PATH=" + filepath.Join(home, ".local", "bin") + ":" + os.Getenv("PATH")})
	if err != nil {
		t.Fatalf("cleanup script failed after a disable error: %v\n%s", err, output)
	}

	calls := readSystemctlCalls(t, home)
	assertSystemctlCallCount(t, calls, "--user disable --now opencode-web.service", 1)
	assertSystemctlCallCount(t, calls, "--user disable --now opencode-web-health.service", 1)
	assertSystemctlCallCount(t, calls, "--user disable --now opencode-web-health.timer", 1)
	assertSystemctlCallCount(t, calls, "--user daemon-reload", 1)
	assertPathRemoved(t, failedLink)
	if !strings.Contains(string(output), "WARNING") {
		t.Fatalf("cleanup script omitted the warning after a disable error:\n%s", output)
	}
}

// Containers and CI hosts may have no systemd, so the script must exit 0
// without invoking anything rather than failing the whole apply. A host with no
// unit files must not warn about a service it never had.
func TestCleanupScriptSkipsWithoutSystemd(t *testing.T) {
	home := t.TempDir()

	output, err := runCleanupScript(t, home, "full", []string{"PATH=" + t.TempDir()})
	if err != nil {
		t.Fatalf("cleanup script failed without systemd: %v\n%s", err, output)
	}
	if _, err := os.Stat(filepath.Join(home, "systemctl-calls")); !os.IsNotExist(err) {
		t.Fatalf("systemctl calls recorded without systemd: %v", err)
	}
	if strings.Contains(string(output), "WARNING") {
		t.Fatalf("cleanup script warned without unit files:\n%s", output)
	}
}

// A host can have the systemctl binary but no running systemd user manager; the
// script must still remove the units' login symlinks, leave unrelated entries
// alone, and warn that the running service may need a manual stop.
func TestCleanupScriptSkipsWithoutUserManager(t *testing.T) {
	home := t.TempDir()
	writeExecutable(t, filepath.Join(home, ".local", "bin", "systemctl"), fakeSystemctlScript("1", ""))
	writeUserUnitFile(t, home, "opencode-web.service")
	writeUserUnitFile(t, home, "opencode-web-health.timer")
	webLink := writeWantsSymlink(t, home, "default.target.wants", "opencode-web.service")
	timerLink := writeWantsSymlink(t, home, "timers.target.wants", "opencode-web-health.timer")
	otherLink := writeWantsSymlink(t, home, "default.target.wants", "other.service")
	regularFile := filepath.Join(home, ".config", "systemd", "user", "default.target.wants", "opencode-web-health.service")
	writeApplyFile(t, regularFile, "not a symlink\n")

	output, err := runCleanupScript(t, home, "full", []string{"PATH=" + filepath.Join(home, ".local", "bin") + ":" + os.Getenv("PATH")})
	if err != nil {
		t.Fatalf("cleanup script failed without a running systemd user manager: %v\n%s", err, output)
	}
	calls := readSystemctlCalls(t, home)
	if strings.Contains(calls, "disable") || strings.Contains(calls, "daemon-reload") {
		t.Fatalf("cleanup ran without a running systemd user manager:\n%s", calls)
	}
	assertPathRemoved(t, webLink)
	assertPathRemoved(t, timerLink)
	assertSymlinkRemains(t, otherLink)
	assertRegularFileRemains(t, regularFile)
	if !strings.Contains(string(output), "WARNING") {
		t.Fatalf("cleanup script omitted the warning without a running systemd user manager:\n%s", output)
	}
}

// The terminal profile never installs the web service, so its script must be an
// empty no-op rather than touching a systemd user manager that may not exist.
func TestCleanupScriptSkipsTerminalProfile(t *testing.T) {
	home := t.TempDir()
	text := renderProfileTemplate(t, cleanupScriptTemplate, "terminal")
	if strings.Contains(text, "systemctl") {
		t.Fatalf("terminal script contains systemctl:\n%s", text)
	}

	output, err := runCleanupScript(t, home, "terminal", nil)
	if err != nil {
		t.Fatalf("terminal cleanup script failed: %v\n%s", err, output)
	}
}

// The script intentionally leaves server.env in place, so it must tell the
// operator that the credential file still exists and needs manual removal.
func TestCleanupScriptNoticesServerEnv(t *testing.T) {
	home := t.TempDir()
	writeExecutable(t, filepath.Join(home, ".local", "bin", "systemctl"), fakeSystemctlScript("0", ""))
	writeApplyFile(t, filepath.Join(home, ".config", "opencode", "server.env"), "OPENCODE_SERVER_USERNAME=opencode\n")

	output, err := runCleanupScript(t, home, "full", []string{"PATH=" + filepath.Join(home, ".local", "bin") + ":" + os.Getenv("PATH")})
	if err != nil {
		t.Fatalf("cleanup script failed: %v\n%s", err, output)
	}
	if !strings.Contains(string(output), "server.env") {
		t.Fatalf("cleanup script omitted the server.env notice:\n%s", output)
	}
}

func fakeSystemctlScript(showEnvironmentExit, failDisableUnit string) string {
	script := `#!/bin/sh
printf '%s\n' "$*" >>"$HOME/systemctl-calls"
case "$*" in
	*show-environment*) exit ` + showEnvironmentExit + ` ;;
`
	if failDisableUnit != "" {
		script += `	*"disable --now ` + failDisableUnit + `"*) exit 1 ;;
`
	}
	return script + `esac
exit 0
`
}

func runCleanupScript(t *testing.T, home, selected string, environment []string) ([]byte, error) {
	t.Helper()
	script := filepath.Join(t.TempDir(), "remove-opencode-web-service.sh")
	contents := renderProfileTemplate(t, cleanupScriptTemplate, selected)
	if err := os.WriteFile(script, []byte(contents), 0o700); err != nil {
		t.Fatal(err)
	}
	command := exec.Command("sh", script)
	command.Env = append(os.Environ(), "HOME="+home, "PDE_PROFILE="+selected)
	command.Env = append(command.Env, environment...)
	return command.CombinedOutput()
}

func writeUserUnitFile(t *testing.T, home, unit string) {
	t.Helper()
	writeApplyFile(t, filepath.Join(home, ".config", "systemd", "user", unit), "[Unit]\nDescription=test\n")
}

func writeWantsSymlink(t *testing.T, home, wantsDir, unit string) string {
	t.Helper()
	dir := filepath.Join(home, ".config", "systemd", "user", wantsDir)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(dir, unit)
	if err := os.Symlink(filepath.Join("..", unit), path); err != nil {
		t.Fatal(err)
	}
	return path
}

func assertPathRemoved(t *testing.T, path string) {
	t.Helper()
	if _, err := os.Lstat(path); !os.IsNotExist(err) {
		t.Fatalf("%s still exists: %v", path, err)
	}
}

func assertSymlinkRemains(t *testing.T, path string) {
	t.Helper()
	info, err := os.Lstat(path)
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("%s mode = %v, want symlink", path, info.Mode())
	}
}

func assertRegularFileRemains(t *testing.T, path string) {
	t.Helper()
	info, err := os.Stat(path)
	if err != nil {
		t.Fatal(err)
	}
	if !info.Mode().IsRegular() {
		t.Fatalf("%s mode = %v, want regular file", path, info.Mode())
	}
}

func readSystemctlCalls(t *testing.T, home string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(home, "systemctl-calls"))
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func assertSystemctlCallCount(t *testing.T, calls, want string, count int) {
	t.Helper()
	got := 0
	for _, line := range strings.Split(calls, "\n") {
		if line == want {
			got++
		}
	}
	if got != count {
		t.Errorf("systemctl call %q count = %d, want %d:\n%s", want, got, count, calls)
	}
}
