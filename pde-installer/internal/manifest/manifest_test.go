package manifest

import "testing"

// Each managed item must have exactly one pinned owner.
func TestManifestAssignsOneOwner(t *testing.T) {
	t.Parallel()
	if err := Validate(); err != nil {
		t.Fatalf("Validate() error = %v", err)
	}
}

// Runtime tools must appear in the installer ownership list.
func TestManifestIncludesRuntimeTools(t *testing.T) {
	t.Parallel()
	for _, name := range []string{"ya", "gopls", "lua-language-server"} {
		item, ok := Find(name, Aqua)
		if !ok || item.Version == "" {
			t.Errorf("Find(%q, Aqua) = %#v, %t", name, item, ok)
		}
	}
}

// Pkgsrc must install only packages assigned to it.
func TestManifestDrivesPkgsrcOrder(t *testing.T) {
	t.Parallel()
	packages := PkgsrcPackages()
	owned := ByOwner(Pkgsrc)
	if len(packages) != len(owned) {
		t.Fatalf("package count = %d, want %d", len(packages), len(owned))
	}
	for index := range owned {
		if packages[index] != owned[index].Name {
			t.Errorf("package %d = %q, want %q", index, packages[index], owned[index].Name)
		}
	}
}
