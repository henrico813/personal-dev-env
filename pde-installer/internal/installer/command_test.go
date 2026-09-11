package installer

import (
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
	command.SetArgs([]string{"install", "full", "--repo-root", testRepositoryRoot(t)})
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
	command.SetArgs([]string{"list", "--repo-root", invalid})
	err := command.Execute()
	if err == nil || !strings.Contains(err.Error(), "invalid --repo-root") {
		t.Fatalf("Execute() error = %v", err)
	}
}

func TestCommandsRecoverProfiles(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("mutating commands intentionally reject UID 0")
	}
	for _, commandName := range []string{"install", "doctor", "list"} {
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
			case "install":
				command = mutatingCommand(commandName, "", &repoRoot, action)
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

func TestInstallRejectsMultipleSelections(t *testing.T) {
	command := NewCommand()
	command.SetArgs([]string{"install", "terminal", "full"})
	if err := command.Execute(); err == nil || !strings.Contains(err.Error(), "accepts at most 1 arg") {
		t.Fatalf("install arguments error = %v", err)
	}
}

// Root execution violates the installer's user-owned path model.
func TestMutationsRejectRootExecution(t *testing.T) {
	if err := rejectUID(0, "install"); err == nil {
		t.Fatal("rejectUID(0, install) error = nil")
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
