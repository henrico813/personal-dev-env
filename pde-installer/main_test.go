package main_test

import (
	"bytes"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"testing"

	"pde-installer/internal/fsutil"
	"pde-installer/internal/installer"
)

func TestCommandSecurityBoundaries(t *testing.T) {
	repo := repositoryRoot(t)
	if os.Geteuid() == 0 {
		_, _, err := execute(t, t.TempDir(), "install", "--dry-run", "--repo-root", repo)
		if err == nil || !strings.Contains(err.Error(), "refuses UID 0") {
			t.Fatalf("root error = %v", err)
		}
		return
	}

	t.Run("dry run leaves home unchanged", func(t *testing.T) {
		home := t.TempDir()
		before := treeState(t, home)
		stdout, _, err := execute(t, home, "install", "--dry-run", "--repo-root", repo)
		if err != nil {
			t.Fatal(err)
		}
		if after := treeState(t, home); after != before {
			t.Fatalf("HOME changed:\nbefore: %s\nafter:  %s", before, after)
		}
		ordered := []string{"atomically activate tmux", "install Aqua packages", "direct release tools", "materialize complete npm lock", "managed fonts", "activate planner", "preview chezmoi changes", "complete blink.cmp plugin tree"}
		position := -1
		for _, text := range ordered {
			next := strings.Index(stdout[position+1:], text)
			if next < 0 {
				t.Fatalf("missing ordered action %q in:\n%s", text, stdout)
			}
			position += next + 1
		}
	})

	t.Run("symlink escape is rejected", func(t *testing.T) {
		home, outside := t.TempDir(), t.TempDir()
		if err := os.Symlink(outside, filepath.Join(home, ".local")); err != nil {
			t.Fatal(err)
		}
		_, _, err := execute(t, home, "config", "--dry-run", "--repo-root", repo)
		if err == nil || !strings.Contains(err.Error(), "outside HOME") {
			t.Fatalf("containment error = %v", err)
		}
	})

	t.Run("atomic activation restores destination", func(t *testing.T) {
		if runtime.GOOS != "linux" {
			t.Skip("atomic exchange requires Linux")
		}
		home := t.TempDir()
		stage := filepath.Join(home, "stage")
		destination := filepath.Join(home, "active")
		writeFile(t, filepath.Join(stage, "content"), "new\n", 0o644)
		writeFile(t, filepath.Join(destination, "content"), "old\n", 0o644)
		journal, err := fsutil.NewJournal(fsutil.JournalConfig{Home: home})
		if err != nil {
			t.Fatal(err)
		}
		if err := journal.Activate(stage, destination); err != nil {
			t.Fatal(err)
		}
		if content := mustReadFile(t, destination); content != "new\n" {
			t.Fatalf("active content = %q", content)
		}
		if content := mustReadFile(t, stage); content != "old\n" {
			t.Fatalf("staged content = %q", content)
		}
		if err := journal.Rollback(); err != nil {
			t.Fatal(err)
		}
		if content := mustReadFile(t, destination); content != "old\n" {
			t.Fatalf("restored content = %q", content)
		}
		if _, err := os.Lstat(stage); !os.IsNotExist(err) {
			t.Fatalf("staging path remains: %v", err)
		}
	})
}

func TestConfigFailureRestoresContent(t *testing.T) {
	if os.Geteuid() == 0 {
		t.Skip("mutating commands intentionally reject UID 0")
	}
	tests := []struct {
		name    string
		symlink bool
	}{
		{name: "regular file"},
		{name: "outside leaf symlink", symlink: true},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			home, repo := t.TempDir(), repositoryRoot(t)
			target := filepath.Join(home, ".config", "opencode", "opencode.json")
			original := "{\"user_setting\":true}\n"
			outside := ""
			if tt.symlink {
				outside = filepath.Join(t.TempDir(), "opencode.json")
				writeFile(t, outside, original, 0o644)
				if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
					t.Fatal(err)
				}
				if err := os.Symlink(outside, target); err != nil {
					t.Fatal(err)
				}
			} else {
				writeFile(t, target, original, 0o644)
			}
			chezmoi := filepath.Join(home, ".local", "share", "aquaproj-aqua", "bin", "chezmoi")
			script := `#!/bin/sh
set -eu
case " $* " in
  *" status "*) printf ' M %s\n' "$HOME/.config/opencode/opencode.json" ;;
  *" apply "*) printf '{"managed":true}\n' > "$HOME/.config/opencode/opencode.json"; exit 23 ;;
esac
`
			writeFile(t, chezmoi, script, 0o755)
			_, _, err := execute(t, home, "config", "--repo-root", repo)
			if err == nil {
				t.Fatal("config failure returned nil")
			}
			if tt.symlink {
				link, readErr := os.Readlink(target)
				if readErr != nil || link != outside {
					t.Fatalf("restored symlink = %q, %v", link, readErr)
				}
			}
			content, readErr := os.ReadFile(target)
			if readErr != nil {
				t.Fatal(readErr)
			}
			if string(content) != original {
				t.Fatalf("restored content = %q", content)
			}
			if tt.symlink {
				content, readErr = os.ReadFile(outside)
				if readErr != nil || string(content) != original {
					t.Fatalf("outside content = %q, %v", content, readErr)
				}
			}
		})
	}
}

func execute(t *testing.T, home string, arguments ...string) (string, string, error) {
	t.Helper()
	t.Setenv("HOME", home)
	var stdout, stderr bytes.Buffer
	command := installer.NewCommand()
	command.SetOut(&stdout)
	command.SetErr(&stderr)
	command.SetArgs(arguments)
	err := command.Execute()
	return stdout.String(), stderr.String(), err
}

func repositoryRoot(t *testing.T) string {
	t.Helper()
	workingDirectory, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	return filepath.Dir(workingDirectory)
}

func treeState(t *testing.T, root string) string {
	t.Helper()
	var entries []string
	err := filepath.Walk(root, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		entries = append(entries, relative+":"+info.Mode().String()+":"+strconv.FormatInt(info.Size(), 10))
		return nil
	})
	if err != nil {
		t.Fatal(err)
	}
	return strings.Join(entries, "\n")
}

func writeFile(t *testing.T, path, content string, mode os.FileMode) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), mode); err != nil {
		t.Fatal(err)
	}
}

func mustReadFile(t *testing.T, root string) string {
	t.Helper()
	content, err := os.ReadFile(filepath.Join(root, "content"))
	if err != nil {
		t.Fatal(err)
	}
	return string(content)
}
