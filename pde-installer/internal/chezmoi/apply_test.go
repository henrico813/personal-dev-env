package chezmoi

import (
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/run"
)

// A committed apply must keep changes and remove backups.
func TestApplyCommitsChanges(t *testing.T) {
	t.Parallel()
	fixture := newApplyFixture(t, "success")
	writeApplyFile(t, fixture.target, "old\n")
	writeApplyFile(t, fixture.state, "old-state\n")

	journal, err := fixture.manager.Apply()
	if err != nil {
		t.Fatal(err)
	}
	assertApplyFile(t, fixture.target, "new\n")
	assertApplyFile(t, fixture.state, "new-state\n")
	if len(journal.Changes) != 2 {
		t.Fatalf("Apply() changes = %#v", journal.Changes)
	}
	for _, change := range journal.Changes {
		if _, err := os.Lstat(change.Backup); err != nil {
			t.Fatalf("inspect backup %q: %v", change.Backup, err)
		}
	}
	if err := journal.Commit(); err != nil {
		t.Fatal(err)
	}
	for _, change := range journal.Changes {
		if _, err := os.Lstat(change.Backup); !os.IsNotExist(err) {
			t.Fatalf("backup %q remains: %v", change.Backup, err)
		}
	}
}

// A failed command must restore files and durable state.
func TestApplyFailureRestoresFiles(t *testing.T) {
	t.Parallel()
	fixture := newApplyFixture(t, "fail")
	writeApplyFile(t, fixture.target, "old\n")
	writeApplyFile(t, fixture.state, "old-state\n")

	if _, err := fixture.manager.Apply(); err == nil || !strings.Contains(err.Error(), "exit status 9") {
		t.Fatalf("Apply() error = %v", err)
	}
	assertApplyFile(t, fixture.target, "old\n")
	assertApplyFile(t, fixture.state, "old-state\n")
	info, err := os.Lstat(fixture.target)
	if err != nil {
		t.Fatal(err)
	}
	if !info.Mode().IsRegular() {
		t.Fatalf("restored target mode = %v", info.Mode())
	}
}

// Status output must never authorize writes outside HOME.
func TestApplyRejectsOutsideHome(t *testing.T) {
	t.Parallel()
	fixture := newApplyFixture(t, "outside")

	if _, err := fixture.manager.Apply(); err == nil || !strings.Contains(err.Error(), "outside HOME") {
		t.Fatalf("Apply() error = %v", err)
	}
	if _, err := os.Lstat(fixture.target); !os.IsNotExist(err) {
		t.Fatalf("target exists after rejected status: %v", err)
	}
}

// Empty status must not invent managed target changes.
func TestApplyHandlesUnchangedStatus(t *testing.T) {
	t.Parallel()
	fixture := newApplyFixture(t, "unchanged")
	writeApplyFile(t, fixture.state, "old-state\n")

	journal, err := fixture.manager.Apply()
	if err != nil {
		t.Fatal(err)
	}
	if len(journal.Changes) != 1 || journal.Changes[0].Destination != fixture.state {
		t.Fatalf("Apply() changes = %#v", journal.Changes)
	}
	if err := journal.Commit(); err != nil {
		t.Fatal(err)
	}
	assertApplyFile(t, fixture.state, "new-state\n")
}

type applyFixture struct {
	manager Manager
	target  string
	state   string
}

func newApplyFixture(t *testing.T, mode string) applyFixture {
	t.Helper()
	root := t.TempDir()
	home := filepath.Join(root, "home")
	repoRoot := filepath.Join(root, "repo")
	aquaRoot := filepath.Join(home, ".local", "share", "aquaproj-aqua")
	source := filepath.Join(repoRoot, "chezmoi")
	binary := filepath.Join(aquaRoot, "bin", "chezmoi")

	writeApplyFile(t, filepath.Join(source, ".chezmoiexternal.toml"), "")
	writeApplyFile(t, filepath.Join(source, "dot_config", "opencode", "modify_opencode.json"), "{}\n")
	writeApplyFile(t, filepath.Join(source, "test-mode"), mode+"\n")
	writeExecutable(t, binary, `#!/bin/sh
set -eu
source_dir=
destination=
state=
command=
while [ "$#" -gt 0 ]; do
	case "$1" in
		--source) source_dir=$2; shift 2 ;;
		--destination) destination=$2; shift 2 ;;
		--persistent-state) state=$2; shift 2 ;;
		status|apply) command=$1; shift ;;
		*) shift ;;
	esac
done
mode=$(cat "$source_dir/test-mode")
target="$destination/.config/tool"
case "$command" in
	status)
		case "$mode" in
			unchanged) exit 0 ;;
			outside) printf 'M  %s\n' "$source_dir/../outside"; exit 0 ;;
		esac
		if [ ! -f "$target" ] || [ "$(cat "$target")" != new ]; then
			printf 'M  %s\n' "$target"
		fi
		;;
	apply)
		printf 'new-state\n' >"$state"
		if [ "$mode" != unchanged ]; then
			mkdir -p "$destination/.config"
			printf 'new\n' >"$target"
		fi
		if [ "$mode" = fail ]; then
			exit 9
		fi
		;;
	esac
`)

	state := filepath.Join(home, ".local", "state", "pde", "chezmoi.boltdb")
	return applyFixture{
		manager: New(home, repoRoot, aquaRoot, run.Runner{Stdout: io.Discard, Stderr: io.Discard}),
		target:  filepath.Join(home, ".config", "tool"),
		state:   state,
	}
}

func writeApplyFile(t *testing.T, path, content string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func writeExecutable(t *testing.T, path, content string) {
	t.Helper()
	writeApplyFile(t, path, content)
	if err := os.Chmod(path, 0o755); err != nil {
		t.Fatal(err)
	}
}

func assertApplyFile(t *testing.T, path, want string) {
	t.Helper()
	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(content) != want {
		t.Fatalf("%s content = %q, want %q", path, content, want)
	}
}
