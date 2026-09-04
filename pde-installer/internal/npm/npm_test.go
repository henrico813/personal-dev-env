package npm

import (
	"bytes"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/manifest"
	"pde-installer/internal/run"
)

// npm must install the versions assigned by the manifest.
func TestNPMUsesManifestVersions(t *testing.T) {
	t.Parallel()
	for _, spec := range packages() {
		item, ok := manifest.Find(spec.Name, manifest.NPM)
		if !ok || spec.Version != item.Version {
			t.Errorf("package %q version = %q, want %q", spec.Name, spec.Version, item.Version)
		}
	}
}

// The checked-in lock must include every pinned dependency.
func TestNPMLockIncludesPinnedPackages(t *testing.T) {
	t.Parallel()
	repoRoot, err := filepath.Abs(filepath.Join("..", "..", ".."))
	if err != nil {
		t.Fatal(err)
	}
	if err := New(t.TempDir(), repoRoot, run.Runner{}).ValidateLock(); err != nil {
		t.Fatalf("ValidateLock() error = %v", err)
	}
}

// Reconciliation must activate exact packages and preserve prior installs.
func TestReconcileActivatesAndRollsBack(t *testing.T) {
	home := t.TempDir()
	repoRoot, err := filepath.Abs(filepath.Join("..", "..", ".."))
	if err != nil {
		t.Fatal(err)
	}
	logPath := filepath.Join(t.TempDir(), "npm.log")
	writeNPMFixture(t, filepath.Join(home, ".local", "bin", "npm"), logPath)
	manager := New(home, repoRoot, run.Runner{})

	root := manager.Root()
	writeNPMFile(t, filepath.Join(root, "previous"), "old root\n", 0o644)
	for _, spec := range packages() {
		writeNPMFile(t, filepath.Join(home, ".local", "bin", spec.Binary), "old "+spec.Binary+"\n", 0o755)
	}

	journal, err := manager.Reconcile()
	if err != nil {
		t.Fatalf("Reconcile() error = %v", err)
	}
	assertNPMInstall(t, manager)
	if err := journal.Rollback(); err != nil {
		t.Fatalf("Rollback() error = %v", err)
	}
	assertNPMFile(t, filepath.Join(root, "previous"), "old root\n")
	for _, spec := range packages() {
		assertNPMFile(t, filepath.Join(home, ".local", "bin", spec.Binary), "old "+spec.Binary+"\n")
	}

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
		t.Fatalf("unchanged Reconcile() reran npm: before %q, after %q", before, after)
	}
	writeNPMFile(t, filepath.Join(root, "package-lock.json"), "changed\n", 0o644)
	journal, err = manager.Reconcile()
	if err != nil {
		t.Fatalf("changed lock Reconcile() error = %v", err)
	}
	if len(journal.Changes) == 0 {
		t.Fatal("changed lock did not rerun npm")
	}
	if err := journal.Rollback(); err != nil {
		t.Fatalf("changed lock Rollback() error = %v", err)
	}
}

func writeNPMFixture(t *testing.T, path, logPath string) {
	t.Helper()
	var script strings.Builder
	fmt.Fprintf(&script, "#!/bin/sh\nset -eu\nprintf '%%s\\n' \"$1\" >> %q\n", logPath)
	script.WriteString("[ \"$1\" = ci ] || exit 0\nmkdir -p node_modules/.bin\n")
	for _, spec := range packages() {
		metadata := filepath.Join("node_modules", filepath.FromSlash(spec.Name), "package.json")
		binary := filepath.Join("node_modules", ".bin", spec.Binary)
		fmt.Fprintf(&script, "mkdir -p %q\nprintf '%%s\\n' '{\"version\":\"%s\"}' > %q\n", filepath.Dir(metadata), spec.Version, metadata)
		fmt.Fprintf(&script, "printf '%%s\\n' '#!/bin/sh' 'printf \"%%s\\\\n\" \"%s\"' > %q\nchmod +x %q\n", spec.Version, binary, binary)
	}
	writeNPMFile(t, path, script.String(), 0o755)
}

func assertNPMInstall(t *testing.T, manager Manager) {
	t.Helper()
	for _, spec := range packages() {
		version, err := manager.Version(spec.Name)
		if err != nil {
			t.Fatal(err)
		}
		if version != spec.Version {
			t.Errorf("Version(%q) = %q, want %q", spec.Name, version, spec.Version)
		}
		launcher := filepath.Join(manager.Home, ".local", "bin", spec.Binary)
		target, err := os.Readlink(launcher)
		if err != nil {
			t.Fatalf("Readlink(%q) error = %v", launcher, err)
		}
		wantTarget := filepath.Join(manager.Root(), "node_modules", ".bin", spec.Binary)
		if target != wantTarget {
			t.Errorf("launcher target = %q, want %q", target, wantTarget)
		}
		output, err := manager.Runner.Query("check fixture", run.Command{Name: launcher, Args: []string{"--version"}})
		if err != nil {
			t.Fatal(err)
		}
		if got := strings.TrimSpace(string(output)); got != spec.Version {
			t.Errorf("%s version = %q, want %q", spec.Binary, got, spec.Version)
		}
	}
}

func writeNPMFile(t *testing.T, path, content string, mode os.FileMode) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(content), mode); err != nil {
		t.Fatal(err)
	}
}

func assertNPMFile(t *testing.T, path, want string) {
	t.Helper()
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if string(data) != want {
		t.Fatalf("%s = %q, want %q", path, data, want)
	}
}
