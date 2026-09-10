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

// Reconciliation must activate builds and preserve prior binaries.
func TestReconcileBuildsAndRollsBack(t *testing.T) {
	home := t.TempDir()
	repoRoot := t.TempDir()
	for _, source := range []string{"planner", "cli", "surveil", "vibe"} {
		writeBuildFile(t, filepath.Join(repoRoot, source, "input.txt"), source+" source\n", 0o644)
	}
	logPath := filepath.Join(t.TempDir(), "build.log")
	writeBuildFixture(t, filepath.Join(home, ".local", "bin", "go"), goFixture(logPath))
	writeBuildFixture(t, filepath.Join(home, ".local", "bin", "cargo"), cargoFixture(logPath))
	manager := New(home, repoRoot, run.Runner{})

	for _, name := range []string{"planner", "opencode-inline-shim", "surveil", "vibe"} {
		writeBuildFile(t, filepath.Join(home, ".local", "bin", name), "old "+name+"\n", 0o755)
	}
	writeBuildFile(t, manager.statePath(), "old state\n", 0o644)

	journal, err := manager.Reconcile()
	if err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	assertBuildOutputs(t, home)
	assertBuildArtifactsRemoved(t, filepath.Join(home, ".local", "state", "pde", "build-stage"))
	if err := journal.Rollback(); err != nil {
		t.Fatalf("Rollback() error = %v", err)
	}
	for _, name := range []string{"planner", "opencode-inline-shim", "surveil", "vibe"} {
		assertBuildFile(t, filepath.Join(home, ".local", "bin", name), "old "+name+"\n")
	}
	assertBuildFile(t, manager.statePath(), "old state\n")

	journal, err = manager.Reconcile()
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
	journal, err = manager.Reconcile()
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
mkdir -p "$GOCACHE" "$GOMODCACHE"
printf '%%s\n' cache > "$GOCACHE/content"
printf '%%s\n' module > "$GOMODCACHE/content"
mkdir -p "$(dirname "$output")"
printf '%%s\n' '#!/bin/sh' 'exit 0' '# built by go' > "$output"
chmod +x "$output"
`, shellQuote(logPath))
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
		"planner": "go", "opencode-inline-shim": "go", "surveil": "cargo", "vibe": "cargo",
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
		filepath.Join(stageRoot, "surveil-target"),
		filepath.Join(stageRoot, "vibe-target"),
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
