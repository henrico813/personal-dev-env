package chezmoi

import (
	"io"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/profile"
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

func TestFullApplyPreservesTerminalFiles(t *testing.T) {
	fixture := newApplyFixture(t, "unchanged")
	terminalConfig := filepath.Join(fixture.manager.Home, ".config", "aquaproj-aqua", "aqua-terminal.yaml")
	terminalChecksums := filepath.Join(fixture.manager.Home, ".config", "aquaproj-aqua", "aqua-terminal-checksums.json")
	writeApplyFile(t, terminalConfig, "terminal manifest\n")
	writeApplyFile(t, terminalChecksums, "terminal checksums\n")
	configBefore, err := os.ReadFile(terminalConfig)
	if err != nil {
		t.Fatal(err)
	}
	checksumsBefore, err := os.ReadFile(terminalChecksums)
	if err != nil {
		t.Fatal(err)
	}
	fullTarget := filepath.Join(fixture.manager.Home, ".config", "full-only")
	writeApplyFile(t, filepath.Join(fixture.manager.Source(), "test-profile"), "full\n")
	writeExecutable(t, filepath.Join(fixture.manager.AquaRoot, "bin", "chezmoi"), `#!/bin/sh
set -eu
source_dir=
destination=
command=
while [ "$#" -gt 0 ]; do
	case "$1" in
		--source) source_dir=$2; shift 2 ;;
		--destination) destination=$2; shift 2 ;;
		status|apply) command=$1; shift ;;
		*) shift ;;
	esac
done
[ "$PDE_PROFILE" = full ]
[ "$AQUA_GLOBAL_CONFIG" = "$source_dir/dot_config/aquaproj-aqua/aqua.yaml" ]
[ "$AQUA_CHECKSUMS_PATH" = "$source_dir/dot_config/aquaproj-aqua/aqua-checksums.json" ]
target="$destination/.config/full-only"
case "$command" in
	status)
		if [ ! -f "$target" ]; then printf 'M  %s\n' "$target"; fi
		;;
	apply)
		mkdir -p "$(dirname "$target")"
		printf 'managed by full\n' >"$target"
		;;
esac
`)
	fixture.manager = New(fixture.manager.Home, fixture.manager.RepoRoot, fixture.manager.AquaRoot, profile.Full, run.Runner{Stdout: io.Discard, Stderr: io.Discard})
	journal, err := fixture.manager.Apply()
	if err != nil {
		t.Fatal(err)
	}
	if journal == nil {
		t.Fatal("Apply() returned nil journal")
	}
	assertApplyFile(t, terminalConfig, string(configBefore))
	assertApplyFile(t, terminalChecksums, string(checksumsBefore))
	assertApplyFile(t, fullTarget, "managed by full\n")
}

func TestApplySetsProfileEnvironment(t *testing.T) {
	for _, selected := range []profile.Profile{profile.Full, profile.Terminal} {
		t.Run(string(selected), func(t *testing.T) {
			fixture := newApplyFixtureForProfile(t, "unchanged", selected)
			if _, err := fixture.manager.Apply(); err != nil {
				t.Fatal(err)
			}
		})
	}
}

func TestApplyRejectsMissingTemplates(t *testing.T) {
	for _, name := range []string{".chezmoiignore.tmpl", "dot_zshrc.tmpl", "dot_tmux.conf.tmpl"} {
		t.Run(name, func(t *testing.T) {
			fixture := newApplyFixture(t, "success")
			if err := os.Remove(filepath.Join(fixture.manager.Source(), name)); err != nil {
				t.Fatal(err)
			}
			if _, err := fixture.manager.Apply(); err == nil {
				t.Fatal("Apply() succeeded with missing template")
			}
			if _, err := os.Stat(fixture.state); !os.IsNotExist(err) {
				t.Fatalf("fake apply ran; state stat error = %v", err)
			}
		})
	}
}

func TestApplyRejectsMissingAquaInputs(t *testing.T) {
	tests := []struct {
		name     string
		profile  profile.Profile
		filename string
	}{
		{name: "full manifest", profile: profile.Full, filename: "aqua.yaml"},
		{name: "full checksums", profile: profile.Full, filename: "aqua-checksums.json"},
		{name: "terminal manifest", profile: profile.Terminal, filename: "aqua-terminal.yaml"},
		{name: "terminal checksums", profile: profile.Terminal, filename: "aqua-terminal-checksums.json"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			fixture := newApplyFixtureForProfile(t, "success", test.profile)
			path := filepath.Join(fixture.manager.Source(), "dot_config", "aquaproj-aqua", test.filename)
			if err := os.Remove(path); err != nil {
				t.Fatal(err)
			}
			if _, err := fixture.manager.Apply(); err == nil || !strings.Contains(err.Error(), path) {
				t.Fatalf("Apply() error = %v", err)
			}
			if _, err := os.Stat(fixture.state); !os.IsNotExist(err) {
				t.Fatalf("fake apply ran; state stat error = %v", err)
			}
		})
	}
}

func TestApplyRejectsInvalidProfile(t *testing.T) {
	fixture := newApplyFixtureForProfile(t, "success", profile.Profile("desktop"))
	if _, err := fixture.manager.Apply(); err == nil || !strings.Contains(err.Error(), `invalid profile "desktop"`) {
		t.Fatalf("Apply() error = %v", err)
	}
}

func newApplyFixture(t *testing.T, mode string) applyFixture {
	return newApplyFixtureForProfile(t, mode, profile.Full)
}

func newApplyFixtureForProfile(t *testing.T, mode string, selected profile.Profile) applyFixture {
	t.Helper()
	root := t.TempDir()
	home := filepath.Join(root, "home")
	repoRoot := filepath.Join(root, "repo")
	aquaRoot := filepath.Join(home, ".local", "share", "aquaproj-aqua")
	source := filepath.Join(repoRoot, "chezmoi")
	binary := filepath.Join(aquaRoot, "bin", "chezmoi")

	writeApplyFile(t, filepath.Join(source, ".chezmoiexternal.toml.tmpl"), "")
	writeApplyFile(t, filepath.Join(source, ".chezmoiignore.tmpl"), "")
	writeApplyFile(t, filepath.Join(source, "dot_zshrc.tmpl"), "")
	writeApplyFile(t, filepath.Join(source, "dot_tmux.conf.tmpl"), "")
	writeApplyFile(t, filepath.Join(source, "dot_config", "aquaproj-aqua", "aqua.yaml"), "registries: []\n")
	writeApplyFile(t, filepath.Join(source, "dot_config", "aquaproj-aqua", "aqua-checksums.json"), "{}\n")
	writeApplyFile(t, filepath.Join(source, "dot_config", "aquaproj-aqua", "aqua-terminal.yaml"), "registries: []\n")
	writeApplyFile(t, filepath.Join(source, "dot_config", "aquaproj-aqua", "aqua-terminal-checksums.json"), "{}\n")
	writeApplyFile(t, filepath.Join(source, "dot_config", "opencode", "modify_opencode.json"), "{}\n")
	writeApplyFile(t, filepath.Join(source, "dot_config", "opencode", "modify_opencode-mem.jsonc"), "{}\n")
	writeApplyFile(t, filepath.Join(source, "test-mode"), mode+"\n")
	writeApplyFile(t, filepath.Join(source, "test-profile"), string(selected)+"\n")
	writeExecutable(t, binary, `#!/bin/sh
set -eu
source_dir=
destination=
state=
command=
force=false
while [ "$#" -gt 0 ]; do
	case "$1" in
		--source) source_dir=$2; shift 2 ;;
		--destination) destination=$2; shift 2 ;;
		--persistent-state) state=$2; shift 2 ;;
		--force) force=true; shift ;;
		status|apply) command=$1; shift ;;
		*) shift ;;
	esac
done
: "${AQUA_ROOT_DIR:?}"
expected_profile=$(cat "$source_dir/test-profile")
[ "$PDE_PROFILE" = "$expected_profile" ]
case "$expected_profile" in
	full) aqua_name=aqua ; checksums_name=aqua-checksums ;;
	terminal) aqua_name=aqua-terminal ; checksums_name=aqua-terminal-checksums ;;
	*) exit 8 ;;
esac
[ "$AQUA_GLOBAL_CONFIG" = "$source_dir/dot_config/aquaproj-aqua/$aqua_name.yaml" ]
[ "$AQUA_CHECKSUMS_PATH" = "$source_dir/dot_config/aquaproj-aqua/$checksums_name.json" ]
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
		[ "$force" = true ]
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
		manager: New(home, repoRoot, aquaRoot, selected, run.Runner{Stdout: io.Discard, Stderr: io.Discard}),
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
