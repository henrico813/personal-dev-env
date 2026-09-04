package installer

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// Concurrent installers can corrupt shared files and package state.
func TestCommandBlocksConcurrentInstalls(t *testing.T) {
	home := t.TempDir()
	t.Setenv("HOME", home)
	lock, err := acquireInstallerLock(home)
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := lock.Close(); err != nil {
			t.Error(err)
		}
	})

	command := NewCommand()
	command.SetArgs([]string{"config", "--repo-root", testRepositoryRoot(t)})
	err = command.Execute()
	if err == nil || !strings.Contains(err.Error(), "already running") {
		t.Fatalf("Execute() error = %v", err)
	}
}

// An explicit repository must never fall back to another checkout.
func TestCommandRejectsInvalidRepository(t *testing.T) {
	t.Setenv("HOME", t.TempDir())
	t.Setenv("PDE_REPO_ROOT", testRepositoryRoot(t))
	invalid := filepath.Join(t.TempDir(), "missing")

	command := NewCommand()
	command.SetOut(&bytes.Buffer{})
	command.SetErr(&bytes.Buffer{})
	command.SetArgs([]string{"list", "--repo-root", invalid})
	err := command.Execute()
	if err == nil || !strings.Contains(err.Error(), "invalid --repo-root") {
		t.Fatalf("Execute() error = %v", err)
	}
}

// Root execution violates the installer's user-owned path model.
func TestMutationsRejectRootExecution(t *testing.T) {
	for _, command := range []string{"install", "update", "config"} {
		t.Run(command, func(t *testing.T) {
			if err := rejectUID(0, command); err == nil {
				t.Fatalf("rejectUID(0, %q) error = nil", command)
			}
		})
	}
}

func testRepositoryRoot(t *testing.T) string {
	t.Helper()
	directory, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	return filepath.Clean(filepath.Join(directory, "..", "..", ".."))
}
