package npm

import (
	"path/filepath"
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
	if err := New(t.TempDir(), repoRoot, t.TempDir(), run.Runner{}).ValidateLock(); err != nil {
		t.Fatalf("ValidateLock() error = %v", err)
	}
}
