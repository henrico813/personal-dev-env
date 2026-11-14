package ubuntu

import (
	"os"
	"path/filepath"
	"slices"
	"strings"
	"testing"

	"pde-installer/internal/testfile"
)

// A fresh Ubuntu user starts with bash, so new terminals miss the managed
// ~/.zshrc until the install changes the passwd entry.
func TestSetLoginShellRunsUsermod(t *testing.T) {
	manager, bin, log := loginShellManager(t, "/bin/bash", false)

	if err := manager.SetLoginShell("tester"); err != nil {
		t.Fatal(err)
	}

	want := []string{"usermod -s " + filepath.Join(bin, "zsh") + " tester"}
	if lines := fixtureLines(t, log); !slices.Equal(lines, want) {
		t.Fatalf("sudo calls = %q, want %q", lines, want)
	}
}

// Every install reruns this step, so a zsh user must not get a sudo prompt,
// even when the passwd entry names another zsh path than PATH finds.
func TestSetLoginShellKeepsAnyZsh(t *testing.T) {
	manager, _, log := loginShellManager(t, "/bin/zsh", false)

	if err := manager.SetLoginShell("tester"); err != nil {
		t.Fatal(err)
	}

	if lines := fixtureLines(t, log); len(lines) != 0 {
		t.Fatalf("sudo calls = %q, want none", lines)
	}
}

// chsh and PAM refuse a shell that /etc/shells leaves out, so setting one
// could lock the user out of tools that check it.
func TestSetLoginShellWarnsUnlistedZsh(t *testing.T) {
	manager, _, log := loginShellManager(t, "/bin/bash", false)
	writeFixture(t, manager.shellsPath, "/bin/sh\n/bin/bash\n/usr/bin/zsh\n", 0o644)

	if err := manager.SetLoginShell("tester"); err != nil {
		t.Fatal(err)
	}

	if lines := fixtureLines(t, log); len(lines) != 0 {
		t.Fatalf("sudo calls = %q, want none", lines)
	}
	stderr := manager.runner.Err().(*strings.Builder).String()
	if !strings.Contains(stderr, "is not in "+manager.shellsPath) {
		t.Fatalf("stderr = %q, want a warning naming the shells file", stderr)
	}
}

// A dry run on a fresh machine comes before apt installs zsh, and must still
// show the change instead of failing.
func TestSetLoginShellPreviewsMissingZsh(t *testing.T) {
	manager, bin, _ := loginShellManager(t, "/bin/bash", true)
	if err := os.Remove(filepath.Join(bin, "zsh")); err != nil {
		t.Fatal(err)
	}
	t.Setenv("PATH", bin)

	if err := manager.SetLoginShell("tester"); err != nil {
		t.Fatal(err)
	}

	stdout := manager.runner.Out().(*strings.Builder).String()
	if want := "  sudo usermod -s /usr/bin/zsh tester\n"; !strings.Contains(stdout, want) {
		t.Fatalf("dry run = %q, want %q", stdout, want)
	}
}

// Returns a manager whose getent reports shell and whose PATH has a listed fake zsh, plus its bin folder and sudo log.
func loginShellManager(t *testing.T, shell string, dryRun bool) (Manager, string, string) {
	t.Helper()
	manager, log := fixtureManager(t, nil, dryRun)
	root := filepath.Dir(log)
	bin := filepath.Join(root, "bin")
	// Like the real getent, exit 2 for any other key, so a lookup of the wrong user fails.
	getent := "#!/bin/sh\n[ \"$2\" = tester ] || exit 2\nprintf '%s\\n' 'tester:x:1000:1000::/nonexistent:" + shell + "'\n"
	testfile.WriteExecutable(t, filepath.Join(bin, "getent"), getent)
	testfile.WriteExecutable(t, filepath.Join(bin, "zsh"), "#!/bin/sh\nexit 0\n")
	manager.shellsPath = filepath.Join(root, "shells")
	writeFixture(t, manager.shellsPath, "/bin/sh\n"+filepath.Join(bin, "zsh")+"\n", 0o644)
	return manager, bin, log
}
