package builds

import (
	"bytes"
	"os"
	"path/filepath"
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
	_, err := New(home, t.TempDir(), t.TempDir(), run.Runner{}).Probe("planner")
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
	prefix := t.TempDir()
	logPath := filepath.Join(t.TempDir(), "build.log")
	writeBuildFixture(t, filepath.Join(prefix, "bin", "go"), goFixture(logPath))
	writeBuildFixture(t, filepath.Join(prefix, "bin", "cargo"), cargoFixture(logPath))
	manager := New(home, repoRoot, prefix, run.Runner{})

	for _, name := range []string{"planner", "opencode-inline-shim", "surveil", "vibe"} {
		writeBuildFile(t, filepath.Join(home, ".local", "bin", name), "old "+name+"\n", 0o755)
	}
	writeBuildFile(t, manager.statePath(), "old state\n", 0o644)

	journal, err := manager.Reconcile()
	if err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	assertBuildOutputs(t, home)
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
	return "#!/bin/sh\nset -eu\nprintf '%s\\n' go >> " + shellQuote(logPath) + "\n" +
		"output=\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = -o ]; then output=$2; break; fi\n  shift\ndone\n" +
		"mkdir -p \"$(dirname \"$output\")\"\nprintf '%s\\n' '#!/bin/sh' 'exit 0' '# built by go' > \"$output\"\nchmod +x \"$output\"\n"
}

func cargoFixture(logPath string) string {
	return "#!/bin/sh\nset -eu\nprintf '%s\\n' cargo >> " + shellQuote(logPath) + "\n" +
		"target=\nname=${PWD##*/}\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --target-dir) target=$2; shift 2 ;;\n    --bin) name=$2; shift 2 ;;\n    *) shift ;;\n  esac\ndone\n" +
		"output=$target/release/$name\nmkdir -p \"$(dirname \"$output\")\"\nprintf '%s\\n' '#!/bin/sh' 'exit 0' '# built by cargo' > \"$output\"\nchmod +x \"$output\"\n"
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
