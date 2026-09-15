package chezmoi

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
)

func TestOCAAttachesWhenReady(t *testing.T) {
	home := t.TempDir()
	writeOCARuntime(t, home, "printf 'probe\\n' >>\"$HOME/order\"\nexit 0")

	output, err := ocaCommand(home, "oca --session forwarded").CombinedOutput()
	if err != nil {
		t.Fatalf("oca: %v\n%s", err, output)
	}
	if got := readOCAFile(t, filepath.Join(home, "systemctl-arguments")); got != "" {
		t.Fatalf("systemctl arguments = %q", got)
	}
	if got := readOCAFile(t, filepath.Join(home, "order")); got != "probe\nattach\n" {
		t.Fatalf("call order = %q", got)
	}
	want := "attach\nhttp://127.0.0.1:4096\n--dir\n" + home + "\n--session\nforwarded\n"
	if got := readOCAFile(t, filepath.Join(home, "opencode-arguments")); got != want {
		t.Fatalf("opencode arguments = %q", got)
	}
}

func TestOCARecoversBeforeAttach(t *testing.T) {
	home := t.TempDir()
	writeOCARuntime(t, home, "printf 'probe\\n' >>\"$HOME/order\"\nif [ ! -e \"$HOME/probed\" ]; then touch \"$HOME/probed\"; exit 1; fi\nexit 0")

	output, err := ocaCommand(home, "oca").CombinedOutput()
	if err != nil {
		t.Fatalf("oca: %v\n%s", err, output)
	}
	if got := readOCAFile(t, filepath.Join(home, "systemctl-arguments")); got != "--user\nrestart\nopencode-web.service\n" {
		t.Fatalf("systemctl arguments = %q", got)
	}
	if got := readOCAFile(t, filepath.Join(home, "order")); got != "probe\nprobe\nattach\n" {
		t.Fatalf("call order = %q", got)
	}
}

func TestOCARejectsUnhealthyServer(t *testing.T) {
	home := t.TempDir()
	writeOCARuntime(t, home, "printf 'probe\\n' >>\"$HOME/order\"\nexit 1")

	output, err := ocaCommand(home, "oca").CombinedOutput()
	if err == nil {
		t.Fatalf("oca succeeded: %s", output)
	}
	if !strings.Contains(string(output), "OpenCode server did not become ready: http://127.0.0.1:4096") {
		t.Fatalf("oca output = %q", output)
	}
	if got := readOCAFile(t, filepath.Join(home, "systemctl-arguments")); got != "--user\nrestart\nopencode-web.service\n" {
		t.Fatalf("systemctl arguments = %q", got)
	}
	if got := readOCAFile(t, filepath.Join(home, "order")); got != strings.Repeat("probe\n", 11) {
		t.Fatalf("call order = %q", got)
	}
	if got := readOCAFile(t, filepath.Join(home, "opencode-arguments")); got != "" {
		t.Fatalf("opencode arguments = %q", got)
	}
}

func TestOCABypassesOverrideRecovery(t *testing.T) {
	home := t.TempDir()
	writeOCARuntime(t, home, "printf 'probe\\n' >>\"$HOME/order\"\nexit 1")

	output, err := ocaCommand(home, "OPENCODE_ATTACH_URL=http://127.0.0.1:4199 oca").CombinedOutput()
	if err != nil {
		t.Fatalf("oca: %v\n%s", err, output)
	}
	if got := readOCAFile(t, filepath.Join(home, "systemctl-arguments")); got != "" {
		t.Fatalf("systemctl arguments = %q", got)
	}
	if got := readOCAFile(t, filepath.Join(home, "order")); got != "attach\n" {
		t.Fatalf("call order = %q", got)
	}
	if got := readOCAFile(t, filepath.Join(home, "opencode-arguments")); !strings.HasPrefix(got, "attach\nhttp://127.0.0.1:4199\n") {
		t.Fatalf("opencode arguments = %q", got)
	}
}

func ocaCommand(home, script string) *exec.Cmd {
	command := exec.Command("zsh", "-fc", "source \"$HOME/.zshrc\"; "+script)
	command.Dir = home
	command.Env = append(os.Environ(), "HOME="+home, "PATH="+filepath.Join(home, ".local", "bin")+":"+os.Getenv("PATH"))
	return command
}

func writeOCARuntime(t *testing.T, home, curlScript string) {
	t.Helper()
	writeApplyFile(t, filepath.Join(home, ".zshrc"), renderProfileTemplate(t, "dot_zshrc.tmpl", "full"))
	writeExecutable(t, filepath.Join(home, ".local", "bin", "curl"), "#!/bin/sh\n"+curlScript+"\n")
	writeExecutable(t, filepath.Join(home, ".local", "bin", "systemctl"), "#!/bin/sh\nprintf '%s\\n' \"$@\" >>\"$HOME/systemctl-arguments\"\n")
	writeExecutable(t, filepath.Join(home, ".local", "bin", "sleep"), "#!/bin/sh\nexit 0\n")
	writeExecutable(t, filepath.Join(home, ".local", "bin", "opencode"), "#!/bin/sh\nprintf 'attach\\n' >>\"$HOME/order\"\nprintf '%s\\n' \"$@\" >\"$HOME/opencode-arguments\"\n")
}

func readOCAFile(t *testing.T, path string) string {
	t.Helper()
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return ""
	}
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}
