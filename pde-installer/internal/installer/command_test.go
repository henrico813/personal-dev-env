package installer

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/spf13/cobra"
	"pde-installer/internal/fsutil"
	"pde-installer/internal/profile"
	"pde-installer/internal/run"
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

func TestCommandsRecoverSavedProfile(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("mutating commands intentionally reject UID 0")
	}
	for _, commandName := range []string{"update", "doctor", "list"} {
		t.Run(commandName, func(t *testing.T) {
			home := t.TempDir()
			t.Setenv("HOME", home)
			path := filepath.Join(home, ".config", "pde", "config.json")
			writeCommandTestFile(t, path, `{"profile":"full"}`+"\n")
			stage := filepath.Join(home, ".config", "pde", "recovered-config.json")
			writeCommandTestFile(t, stage, `{"profile":"terminal"}`+"\n")
			journal, err := fsutil.NewJournal(fsutil.JournalConfig{Home: home})
			if err != nil {
				t.Fatal(err)
			}
			if err := journal.Activate(stage, path); err != nil {
				t.Fatal(err)
			}

			var got profile.Profile
			action := func(config config, _ run.Runner) error {
				got = config.Profile
				return nil
			}
			repoRoot := testRepositoryRoot(t)
			var command *cobra.Command
			switch commandName {
			case "update":
				command = mutatingCommand(commandName, "", &repoRoot, requireProfile, nil, action)
			case "doctor", "list":
				command = readCommand(commandName, "", &repoRoot, action)
			}
			if err := command.Execute(); err != nil {
				t.Fatal(err)
			}
			if got != profile.Full {
				t.Fatalf("recovered profile = %q, want %q", got, profile.Full)
			}
		})
	}
}

func writeCommandTestFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestProfileFlagOnlyInstall(t *testing.T) {
	command := NewCommand()
	command.SetArgs([]string{"update", "--profile", "terminal"})
	if err := command.Execute(); err == nil || !strings.Contains(err.Error(), "unknown flag") {
		t.Fatalf("update profile flag error = %v", err)
	}
}

func TestFullProfileDowngradeDoesNotWrite(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("mutating commands intentionally reject UID 0")
	}
	home := t.TempDir()
	t.Setenv("HOME", home)
	path := filepath.Join(home, ".config", "pde", "config.json")
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	original := []byte(`{"profile":"full"}` + "\n")
	if err := os.WriteFile(path, original, 0o644); err != nil {
		t.Fatal(err)
	}
	command := NewCommand()
	command.SetArgs([]string{"install", "--profile", "terminal", "--repo-root", testRepositoryRoot(t)})
	err := command.Execute()
	if err == nil || !strings.Contains(err.Error(), "cannot change profile") {
		t.Fatalf("install downgrade error = %v", err)
	}
	got, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(got, original) {
		t.Fatalf("config changed to %q", got)
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
