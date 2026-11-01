package builds

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"fmt"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/profile"
	"pde-installer/internal/run"
)

// A blocked binary path is broken rather than missing.
func TestBuildProbeReturnsFilesystemErrors(t *testing.T) {
	t.Parallel()
	home := t.TempDir()
	blockingPath := filepath.Join(home, ".local", "bin")
	if err := os.MkdirAll(filepath.Dir(blockingPath), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(blockingPath, []byte("blocked"), 0o644); err != nil {
		t.Fatal(err)
	}
	_, err := New(home, t.TempDir(), run.Runner{}).Probe("planner")
	if err == nil {
		t.Fatal("Probe() error = nil")
	}
}

// Reconciliation must activate builds and preserve prior binaries. If a later
// install step fails, the installer rolls back, so the old files must return;
// on upgraded machines the old pde-gh-write and pde-pr-approve are the bash
// scripts chezmoi used to install. An unchanged rerun must not rebuild.
func TestReconcileBuildsAndRollsBack(t *testing.T) {
	home := t.TempDir()
	repoRoot := t.TempDir()
	writeBuildSources(t, repoRoot)
	// Fake go, cargo and python scripts write placeholder binaries and log each build.
	logPath := filepath.Join(t.TempDir(), "build.log")
	manager := newFixtureManager(t, home, repoRoot, logPath)

	// Files from the previous install, including a plain-file pde-pr-approve.
	for _, name := range []string{
		"planner", "opencode-inline-shim", "vibe", "pde-gh-write", "pde-pr-approve", "pde-comment-review", "skill-lessons",
		"pde-comment-checker", "pde-comment-review-next",
	} {
		writeBuildFile(t, filepath.Join(home, ".local", "bin", name), "old "+name+"\n", 0o755)
	}
	writeBuildFile(t, manager.statePath(), "old state\n", 0o644)

	journal, err := manager.Reconcile(profile.Full)
	if err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	assertBuildOutputs(t, home)
	alias, err := os.Readlink(filepath.Join(home, ".local", "bin", "pde-pr-approve"))
	if err != nil {
		t.Fatalf("pde-pr-approve is not a symlink: %v", err)
	}
	if alias != filepath.Join(home, ".local", "bin", "pde-gh-write") {
		t.Fatalf("pde-pr-approve -> %q, want pde-gh-write", alias)
	}
	assertBuildArtifactsRemoved(t, filepath.Join(home, ".local", "state", "pde", "build-stage"))
	// Roll back as a failed later install step would.
	if err := journal.Rollback(); err != nil {
		t.Fatalf("Rollback() error = %v", err)
	}
	for _, name := range []string{
		"planner", "opencode-inline-shim", "vibe", "pde-gh-write", "pde-pr-approve", "pde-comment-review", "skill-lessons",
		"pde-comment-checker", "pde-comment-review-next",
	} {
		assertBuildFile(t, filepath.Join(home, ".local", "bin", name), "old "+name+"\n")
	}
	assertBuildFile(t, manager.statePath(), "old state\n")

	// Build and commit, then run again with nothing changed.
	journal, err = manager.Reconcile(profile.Full)
	if err != nil {
		t.Fatalf("second Reconcile() error = %v", err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatalf("Commit() error = %v", err)
	}
	before, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatal(err)
	}
	journal, err = manager.Reconcile(profile.Full)
	if err != nil {
		t.Fatalf("unchanged Reconcile() error = %v", err)
	}
	if len(journal.Changes) != 0 {
		t.Fatalf("unchanged Reconcile() changes = %d, want 0", len(journal.Changes))
	}
	after, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(after, before) {
		t.Fatalf("unchanged Reconcile() rebuilt tools: before %q, after %q", before, after)
	}

	// A tool left out of the source hashes would never rebuild after a change.
	writeBuildFile(t, filepath.Join(repoRoot, "pde-comment-review-rs", "input.txt"), "changed\n", 0o644)
	journal, err = manager.Reconcile(profile.Full)
	if err != nil {
		t.Fatalf("changed Reconcile() error = %v", err)
	}
	if len(journal.Changes) == 0 {
		t.Fatal("changed Rust review source did not rebuild")
	}
}

// Herdr's approval popup and its Ctrl+b a key run pde-pr-approve, a link to
// pde-gh-write. When only the link is gone, the next install must bring it
// back even though no source changed, or approvals in Herdr stop opening.
// With the check that does this deleted, every other installer test passes.
func TestReconcileRestoresMissingApproveLink(t *testing.T) {
	home := t.TempDir()
	repoRoot := t.TempDir()
	writeBuildSources(t, repoRoot)
	logPath := filepath.Join(t.TempDir(), "build.log")
	manager := newFixtureManager(t, home, repoRoot, logPath)
	// A finished install, then only the link is deleted.
	journal, err := manager.Reconcile(profile.Full)
	if err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatalf("Commit() error = %v", err)
	}
	link := filepath.Join(home, ".local", "bin", "pde-pr-approve")
	if err := os.Remove(link); err != nil {
		t.Fatal(err)
	}

	journal, err = manager.Reconcile(profile.Full)

	if err != nil {
		t.Fatalf("second Reconcile() error = %v", err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatalf("second Commit() error = %v", err)
	}
	target, err := os.Readlink(link)
	if err != nil {
		t.Fatalf("pde-pr-approve is missing: %v", err)
	}
	if target != filepath.Join(home, ".local", "bin", "pde-gh-write") {
		t.Fatalf("pde-pr-approve -> %q, want pde-gh-write", target)
	}
}

// A uv install leaves skill-lessons as a link into uv's tool folder. If the
// installer accepted the link, that machine would keep the old tool until
// some other build input changes.
func TestReconcileReplacesLinkedSkillLessons(t *testing.T) {
	manager, _, _ := newCommittedManager(t)
	tool := filepath.Join(manager.Home, ".local", "bin", "skill-lessons")
	uvCopy := filepath.Join(t.TempDir(), "skill-lessons")
	writeBuildFixture(t, uvCopy, "#!/bin/sh\nexit 0\n# installed by uv\n")
	if err := os.Remove(tool); err != nil {
		t.Fatal(err)
	}
	if err := os.Symlink(uvCopy, tool); err != nil {
		t.Fatal(err)
	}

	journal, err := manager.Reconcile(profile.Full)

	if err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatalf("Commit() error = %v", err)
	}
	info, err := os.Lstat(tool)
	if err != nil {
		t.Fatal(err)
	}
	if !info.Mode().IsRegular() {
		t.Fatalf("skill-lessons mode = %s, want a regular file", info.Mode())
	}
	assertBuildFile(t, tool, "#!/bin/sh\nexit 0\n# built by python\n")
}

// Any input change rebuilds every tool, vibe's long Rust build included, so
// Python caches and test edits must not count. A missed source change would
// not reach the machine until some other build input changes.
func TestOnlyPythonInputsTriggerRebuild(t *testing.T) {
	tests := []struct {
		name, path string
		rebuild    bool
	}{
		{"source", "skill-lessons/src/skill_lessons/cli.py", true},
		{"build script", "skill-lessons/build.py", true},
		{"bytecode cache", "skill-lessons/src/skill_lessons/__pycache__/cli.cpython-310.pyc", false},
		{"test", "skill-lessons/tests/test_cli.py", false},
		{
			name:    "checker source in a subfolder",
			path:    "pde-comment-checker/src/comment_checker/languages/go.py",
			rebuild: true,
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			manager, repoRoot, logPath := newCommittedManager(t)
			before := readBuildLog(t, logPath)
			writeBuildFile(t, filepath.Join(repoRoot, tt.path), "changed\n", 0o644)

			journal, err := manager.Reconcile(profile.Full)
			if err != nil {
				t.Fatalf("Reconcile() error = %v", err)
			}
			if err := journal.Commit(); err != nil {
				t.Fatalf("Commit() error = %v", err)
			}

			if rebuilt := readBuildLog(t, logPath) != before; rebuilt != tt.rebuild {
				t.Fatalf("rebuilt = %v, want %v", rebuilt, tt.rebuild)
			}
		})
	}
}

// A build that succeeds but cannot run must not stay installed: the
// --version check fails, and every tool from the previous install comes back.
func TestBrokenSkillLessonsRevertsEveryTool(t *testing.T) {
	home, repoRoot := t.TempDir(), t.TempDir()
	writeBuildSources(t, repoRoot)
	manager := newFixtureManager(t, home, repoRoot, filepath.Join(t.TempDir(), "build.log"))
	writeBuildFixture(t, manager.python, "#!/bin/sh\nprintf '%s\\n' '#!/bin/sh' 'exit 1' > \"$3\"\nchmod +x \"$3\"\n")
	names := []string{
		"planner", "opencode-inline-shim", "vibe", "pde-gh-write", "pde-pr-approve", "pde-comment-review", "skill-lessons",
		"pde-comment-checker", "pde-comment-review-next",
	}
	for _, name := range names {
		writeBuildFile(t, filepath.Join(home, ".local", "bin", name), "old "+name+"\n", 0o755)
	}

	_, err := manager.Reconcile(profile.Full)

	if err == nil || !strings.Contains(err.Error(), "verify skill-lessons") {
		t.Fatalf("Reconcile() error = %v, want the skill-lessons check to fail", err)
	}
	for _, name := range names {
		assertBuildFile(t, filepath.Join(home, ".local", "bin", name), "old "+name+"\n")
	}
}

// Full builds blink.cmp from the installer's pinned URL and checksum, and
// client downloads the same archive through chezmoi. Different pins would give
// the two profiles different blink.cmp releases.
func TestClientBlinkPinMatchesBuildPin(t *testing.T) {
	path := filepath.Join("..", "..", "..", "chezmoi", ".chezmoiexternal.toml.tmpl")
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}

	_, entry, found := strings.Cut(string(data), `[".config/nvim/pack/plugins/start/blink.cmp"]`)
	entry, _, _ = strings.Cut(entry, "\n\n")

	if !found {
		t.Fatal("externals have no blink.cmp entry")
	}
	for _, want := range []string{
		`url = "` + defaultBlinkURL + `"`,
		`checksum.sha256 = "` + defaultBlinkSHA256 + `"`,
	} {
		if !strings.Contains(entry, want) {
			t.Errorf("blink.cmp external lacks %s", want)
		}
	}
}

// Keeps a failed install of the new program from breaking today's review tools.
func TestBrokenNextReviewRestoresEveryTool(t *testing.T) {
	home, repoRoot := t.TempDir(), t.TempDir()
	writeBuildSources(t, repoRoot)
	manager := newFixtureManager(t, home, repoRoot, filepath.Join(t.TempDir(), "build.log"))
	writeBuildFile(t, filepath.Join(repoRoot, "pde-comment-review-rs", "fail-help"), "fail\n", 0o644)
	names := []string{
		"planner", "opencode-inline-shim", "vibe", "pde-gh-write", "pde-pr-approve",
		"pde-comment-review", "pde-comment-review-next", "skill-lessons", "pde-comment-checker",
	}
	for _, name := range names {
		writeBuildFile(t, filepath.Join(home, ".local", "bin", name), "old "+name+"\n", 0o755)
	}
	writeBuildFile(t, manager.statePath(), "old state\n", 0o644)

	_, err := manager.Reconcile(profile.Full)

	if err == nil || !strings.Contains(err.Error(), "verify pde-comment-review-next") {
		t.Fatalf("Reconcile() error = %v, want the new review check to fail", err)
	}
	for _, name := range names {
		assertBuildFile(t, filepath.Join(home, ".local", "bin", name), "old "+name+"\n")
	}
	assertBuildFile(t, manager.statePath(), "old state\n")
	stage := filepath.Join(home, ".local", "state", "pde", "build-stage")
	if _, err := os.Stat(stage); !os.IsNotExist(err) {
		t.Fatalf("build stage remains after failure: %v", err)
	}
}

// A client laptop has no Go or Rust, so any build but skill-lessons, which
// only zips Python files, would fail every client install.
func TestClientBuildsOnlySkillLessons(t *testing.T) {
	home, repoRoot := t.TempDir(), t.TempDir()
	writeBuildSources(t, repoRoot)
	logPath := filepath.Join(t.TempDir(), "build.log")
	manager := newFixtureManager(t, home, repoRoot, logPath)

	journal, err := manager.Reconcile(profile.Client)

	if err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatal(err)
	}
	if got := readBuildLog(t, logPath); got != "python\n" {
		t.Fatalf("build log = %q, want one python build", got)
	}
}

// Returns a manager whose tools are built and committed, with its repository
// root and the log that each fake build appends to.
func newCommittedManager(t *testing.T) (manager Manager, repoRoot, logPath string) {
	t.Helper()
	home := t.TempDir()
	repoRoot = t.TempDir()
	writeBuildSources(t, repoRoot)
	logPath = filepath.Join(t.TempDir(), "build.log")
	manager = newFixtureManager(t, home, repoRoot, logPath)
	journal, err := manager.Reconcile(profile.Full)
	if err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatalf("Commit() error = %v", err)
	}
	return manager, repoRoot, logPath
}

func writeBuildSources(t *testing.T, repoRoot string) {
	t.Helper()
	for _, source := range []string{
		"planner", "nvim-plugins/opencode-inline.nvim", "vibe", "pdev2", "pde-comment-review",
		"pde-comment-review-rs",
	} {
		writeBuildFile(t, filepath.Join(repoRoot, source, "input.txt"), source+" source\n", 0o644)
	}
	root := filepath.Join(repoRoot, "skill-lessons")
	writeBuildFile(t, filepath.Join(root, "build.py"), "build\n", 0o644)
	writeBuildFile(t, filepath.Join(root, "src", "skill_lessons", "cli.py"), "cli\n", 0o644)
	writeBuildFile(t, filepath.Join(root, "tests", "test_cli.py"), "tests\n", 0o644)
	checker := filepath.Join(repoRoot, "pde-comment-checker")
	writeBuildFile(t, filepath.Join(checker, "build.py"), "build\n", 0o644)
	writeBuildFile(
		t,
		filepath.Join(checker, "src", "comment_checker", "languages", "go.py"),
		"go\n",
		0o644,
	)
}

func newFixtureManager(t *testing.T, home, repoRoot, logPath string) Manager {
	t.Helper()
	writeBuildFixture(t, filepath.Join(home, ".local", "bin", "go"), goFixture(logPath))
	writeBuildFixture(t, filepath.Join(home, ".local", "bin", "cargo"), cargoFixture(logPath))
	python := filepath.Join(t.TempDir(), "python3")
	writeBuildFixture(t, python, pythonFixture(logPath))
	manager := New(home, repoRoot, run.Runner{})
	manager.python = python
	return manager
}

func readBuildLog(t *testing.T, logPath string) string {
	t.Helper()
	data, err := os.ReadFile(logPath)
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

// Called as "python3 -I build.py OUTPUT" from a Python tool's folder; other
// leading arguments or a folder without build.py fail the build.
func pythonFixture(logPath string) string {
	return fmt.Sprintf(`#!/bin/sh
set -eu
[ "$1 $2" = "-I build.py" ] || { echo "unexpected arguments: $*" >&2; exit 2; }
[ -f build.py ] || { echo "build.py is not in $PWD" >&2; exit 2; }
printf '%%s\n' python >> %s
printf '%%s\n' '#!/bin/sh' 'exit 0' '# built by python' > "$3"
chmod +x "$3"
`, shellQuote(logPath))
}

func goFixture(logPath string) string {
	return fmt.Sprintf(`#!/bin/sh
set -eu
printf '%%s\n' go >> %s
output=
while [ "$#" -gt 0 ]; do
  if [ "$1" = -o ]; then
    output=$2
    break
  fi
  shift
done
mkdir -p "$GOCACHE" "$GOMODCACHE/module"
printf '%%s\n' cache > "$GOCACHE/content"
printf '%%s\n' module > "$GOMODCACHE/module/content"
# Like Go, leave the module folder read-only unless GOFLAGS has -modcacherw.
case " ${GOFLAGS:-} " in
  *" -modcacherw "*) ;;
  *) chmod a-w "$GOMODCACHE/module" ;;
esac
mkdir -p "$(dirname "$output")"
printf '%%s\n' '#!/bin/sh' 'exit 0' '# built by go' > "$output"
chmod +x "$output"
`, shellQuote(logPath))
}

// A broken plugin symlink is replaced and restored by rollback.
func TestInstallPluginReplacesBrokenSymlink(t *testing.T) {
	home := t.TempDir()
	repoRoot := t.TempDir()
	name := "fixture.nvim"
	source := filepath.Join(repoRoot, "nvim-plugins", name, "lua")
	writeBuildFile(t, filepath.Join(source, "fixture", "init.lua"), "return {}\n", 0o644)
	destination := filepath.Join(home, ".config", "nvim", "pack", "plugins", "start", name)
	if err := os.MkdirAll(filepath.Dir(destination), 0o755); err != nil {
		t.Fatal(err)
	}
	removedSource := filepath.Join(repoRoot, "removed")
	if err := os.Symlink(removedSource, destination); err != nil {
		t.Fatal(err)
	}

	journal, err := New(home, repoRoot, run.Runner{}).InstallPlugin(name)
	if err != nil {
		t.Fatalf("InstallPlugin() error = %v", err)
	}
	info, err := os.Lstat(destination)
	if err != nil {
		t.Fatal(err)
	}
	if !info.IsDir() {
		t.Fatalf("destination mode = %s, want directory", info.Mode())
	}
	assertBuildFile(t, filepath.Join(destination, "lua", "fixture", "init.lua"), "return {}\n")
	if err := journal.Rollback(); err != nil {
		t.Fatalf("Rollback() error = %v", err)
	}
	link, err := os.Readlink(destination)
	if err != nil {
		t.Fatal(err)
	}
	if link != removedSource {
		t.Fatalf("restored symlink = %q, want %q", link, removedSource)
	}
}

// An unchanged plugin installation produces no journal changes.
func TestInstallPluginSkipsUnchangedTree(t *testing.T) {
	home := t.TempDir()
	repoRoot := t.TempDir()
	name := "fixture.nvim"
	source := filepath.Join(repoRoot, "nvim-plugins", name, "lua")
	writeBuildFile(t, filepath.Join(source, "init.lua"), "return {}\n", 0o644)
	manager := New(home, repoRoot, run.Runner{})

	journal, err := manager.InstallPlugin(name)
	if err != nil {
		t.Fatalf("first InstallPlugin() error = %v", err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatalf("Commit() error = %v", err)
	}
	journal, err = manager.InstallPlugin(name)
	if err != nil {
		t.Fatalf("second InstallPlugin() error = %v", err)
	}
	if len(journal.Changes) != 0 {
		t.Fatalf("unchanged InstallPlugin() changes = %d, want 0", len(journal.Changes))
	}
}

// The installed runtime remains after its source tree is removed.
func TestInstallPluginSurvivesSourceRemoval(t *testing.T) {
	home := t.TempDir()
	repoRoot := t.TempDir()
	name := "fixture.nvim"
	source := filepath.Join(repoRoot, "nvim-plugins", name, "lua")
	writeBuildFile(t, filepath.Join(source, "init.lua"), "return {}\n", 0o644)
	manager := New(home, repoRoot, run.Runner{})
	journal, err := manager.InstallPlugin(name)
	if err != nil {
		t.Fatalf("InstallPlugin() error = %v", err)
	}
	if err := journal.Commit(); err != nil {
		t.Fatalf("Commit() error = %v", err)
	}
	if err := os.RemoveAll(filepath.Join(repoRoot, "nvim-plugins")); err != nil {
		t.Fatal(err)
	}
	destination := filepath.Join(home, ".config", "nvim", "pack", "plugins", "start", name)
	assertBuildFile(t, filepath.Join(destination, "lua", "init.lua"), "return {}\n")
}

// Invalid plugin names are rejected before source paths are used.
func TestInstallPluginRejectsInvalidName(t *testing.T) {
	for _, name := range []string{"", ".", "..", "nested/plugin.nvim"} {
		t.Run(name, func(t *testing.T) {
			_, err := New(t.TempDir(), t.TempDir(), run.Runner{}).InstallPlugin(name)
			if err == nil {
				t.Fatal("InstallPlugin() error = nil")
			}
		})
	}
}

func TestBlinkCleanupPreservesRollback(t *testing.T) {
	home := t.TempDir()
	repoRoot := t.TempDir()
	stateDir := filepath.Join(home, ".local", "state", "pde")
	destination := filepath.Join(home, ".config", "nvim", "pack", "plugins", "start", "blink.cmp")

	archive := blinkArchiveFixture(t)
	digest := sha256.Sum256(archive)
	server := httptest.NewServer(http.HandlerFunc(func(response http.ResponseWriter, _ *http.Request) {
		_, _ = response.Write(archive)
	}))
	defer server.Close()

	writeBuildFixture(t, filepath.Join(home, ".local", "bin", "cargo"), blinkCargoFixture())
	writeBuildFixture(t, filepath.Join(home, ".local", "bin", "nvim"), "#!/bin/sh\nexit 0\n")
	writeBuildFile(t, filepath.Join(destination, "old"), "old plugin\n", 0o644)

	manager := New(home, repoRoot, run.Runner{})
	manager.blinkURL = server.URL
	manager.blinkSHA256 = hex.EncodeToString(digest[:])

	journal, err := manager.BuildBlink()
	if err != nil {
		t.Fatalf("BuildBlink() error = %v", err)
	}

	assertBuildFile(t, filepath.Join(destination, "lib", "libblink_cmp_fuzzy.so"), "blink library\n")
	assertBlinkArtifactsRemoved(t, stateDir)

	if err := journal.Rollback(); err != nil {
		t.Fatalf("Rollback() error = %v", err)
	}
	assertBuildFile(t, filepath.Join(destination, "old"), "old plugin\n")
}

func cargoFixture(logPath string) string {
	return fmt.Sprintf(`#!/bin/sh
set -eu
[ "$1 $2 $3" = "build --locked --release" ] || exit 2
printf '%%s\n' cargo >> %s
target=
name=${PWD##*/}
while [ "$#" -gt 0 ]; do
  case "$1" in
    --target-dir) target=$2; shift 2 ;;
    --bin) name=$2; shift 2 ;;
    *) shift ;;
  esac
done
mkdir -p "$CARGO_HOME"
printf '%%s\n' cache > "$CARGO_HOME/content"
output=$target/release/$name
mkdir -p "$(dirname "$output")"
printf '%%s\n' '#!/bin/sh' 'exit 0' '# built by cargo' > "$output"
if [ "$name" = pde-comment-review-next ] && [ -f fail-help ]; then
  printf '%%s\n' '#!/bin/sh' 'exit 1' > "$output"
fi
chmod +x "$output"
`, shellQuote(logPath))
}

func shellQuote(value string) string {
	return "'" + value + "'"
}

func writeBuildFixture(t *testing.T, path, content string) {
	t.Helper()
	writeBuildFile(t, path, content, 0o755)
}

func writeBuildFile(t *testing.T, path, content string, mode os.FileMode) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), mode); err != nil {
		t.Fatal(err)
	}
}

func assertBuildOutputs(t *testing.T, home string) {
	t.Helper()
	for name, tool := range map[string]string{
		"planner": "go", "opencode-inline-shim": "go", "vibe": "cargo", "pde-gh-write": "go",
		"pde-comment-review": "go", "skill-lessons": "python",
		"pde-comment-checker":     "python",
		"pde-comment-review-next": "cargo",
	} {
		assertBuildFile(t, filepath.Join(home, ".local", "bin", name), "#!/bin/sh\nexit 0\n# built by "+tool+"\n")
	}
}

func assertBuildFile(t *testing.T, path, want string) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != want {
		t.Fatalf("%s = %q, want %q", path, data, want)
	}
}

func assertBuildArtifactsRemoved(t *testing.T, stageRoot string) {
	t.Helper()
	for _, path := range []string{
		filepath.Join(stageRoot, "planner"),
		filepath.Join(stageRoot, "opencode-inline-shim"),
		filepath.Join(stageRoot, "vibe-target"),
		filepath.Join(stageRoot, "comment-review-target"),
		filepath.Join(stageRoot, "go-cache"),
		filepath.Join(stageRoot, "go-mod-cache"),
		filepath.Join(stageRoot, "cargo-home"),
	} {
		if _, err := os.Stat(path); !os.IsNotExist(err) {
			t.Fatalf("Stat(%q) error = %v, want not exist", path, err)
		}
	}
}

func assertBlinkArtifactsRemoved(t *testing.T, stateDir string) {
	t.Helper()
	entries, err := os.ReadDir(stateDir)
	if err != nil {
		t.Fatal(err)
	}
	for _, entry := range entries {
		if !entry.IsDir() || !strings.HasPrefix(entry.Name(), ".blink-") {
			continue
		}
		workspace := filepath.Join(stateDir, entry.Name())
		for _, name := range []string{"blink.tar.gz", "target", "cargo-home"} {
			if _, err := os.Stat(filepath.Join(workspace, name)); !os.IsNotExist(err) {
				t.Fatalf("Stat(%q) error = %v, want not exist", filepath.Join(workspace, name), err)
			}
		}
		return
	}
	t.Fatal("blink workspace not found")
}

func blinkCargoFixture() string {
	return `#!/bin/sh
set -eu
target=
while [ "$#" -gt 0 ]; do
  if [ "$1" = --target-dir ]; then
    target=$2
    break
  fi
  shift
done
mkdir -p "$CARGO_HOME" "$target/release"
printf '%s\n' cache > "$CARGO_HOME/cache"
printf '%s\n' 'blink library' > "$target/release/libblink_cmp_fuzzy.so"
chmod +x "$target/release/libblink_cmp_fuzzy.so"
`
}

func blinkArchiveFixture(t *testing.T) []byte {
	t.Helper()
	var compressed bytes.Buffer
	zipper := gzip.NewWriter(&compressed)
	archive := tar.NewWriter(zipper)
	content := []byte("return {}\n")
	header := &tar.Header{
		Name: "blink-fixture/lua/blink/cmp/init.lua",
		Mode: 0o644,
		Size: int64(len(content)),
	}
	if err := archive.WriteHeader(header); err != nil {
		t.Fatal(err)
	}
	if _, err := archive.Write(content); err != nil {
		t.Fatal(err)
	}
	if err := archive.Close(); err != nil {
		t.Fatal(err)
	}
	if err := zipper.Close(); err != nil {
		t.Fatal(err)
	}
	return compressed.Bytes()
}
