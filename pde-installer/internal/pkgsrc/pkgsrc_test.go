package pkgsrc

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/manifest"
	"pde-installer/internal/run"
)

// Modified pkgsrc source must not be trusted as the pinned release.
func TestPkgsrcRejectsChangedSource(t *testing.T) {
	manager := testManager(t)
	writeSourceTree(t, manager)
	if err := manager.writeSourceDigest(); err != nil {
		t.Fatal(err)
	}
	if err := manager.ValidateExtractedSource(); err != nil {
		t.Fatalf("validate unchanged source: %v", err)
	}

	path := filepath.Join(manager.SourceRoot, manifest.PkgsrcPackages()[0], "Makefile")
	writeFile(t, path, []byte("changed\n"), 0o644)
	if err := manager.ValidateExtractedSource(); err == nil || !strings.Contains(err.Error(), "source digest changed") {
		t.Fatalf("ValidateExtractedSource() error = %v, want source drift", err)
	}
}

// Output inside source breaks integrity, and Neovim needs vim-license.
func TestPkgsrcUsesSafeBuildVariables(t *testing.T) {
	manager := testManager(t)
	variables := strings.Join(manager.makeVariables(), "\n")
	if !strings.Contains(variables, "PACKAGES="+manager.packageRoot()) {
		t.Fatalf("make variables do not set package root:\n%s", variables)
	}
	if strings.HasPrefix(manager.packageRoot(), manager.SourceRoot+string(filepath.Separator)) {
		t.Fatalf("package root %q is inside source", manager.packageRoot())
	}
	if !strings.Contains(variables, "ACCEPTABLE_LICENSES+=vim-license") {
		t.Fatalf("make variables do not accept vim-license:\n%s", variables)
	}
}

// Package database failures must stop installation before mutation.
func TestPkgsrcClassifiesProbeResults(t *testing.T) {
	tests := []struct {
		name  string
		exit  string
		state string
		fail  bool
	}{
		{name: "missing exit one", exit: "1", state: "missing"},
		{name: "failure exit two", exit: "2", fail: true},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			manager := testManager(t)
			writeProbeFixtures(t, manager, test.exit)
			status, err := manager.packageStatus(manifest.PkgsrcPackages()[0])
			if test.fail {
				if err == nil {
					t.Fatal("packageStatus() error = nil, want failure")
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			if status.State != test.state {
				t.Fatalf("state = %q, want %q", status.State, test.state)
			}
		})
	}
}

// Package state determines whether pkgsrc skips, installs, or replaces.
func TestPkgsrcChoosesPackageAction(t *testing.T) {
	tests := []struct {
		name      string
		installed string
		action    string
	}{
		{name: "install missing", action: "install"},
		{name: "replace outdated", installed: "tool-0", action: "replace"},
		{name: "skip current", installed: "tool-1"},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			manager := reconcileManager(t, test.installed, "tool-1", false)
			if err := manager.Reconcile(); err != nil {
				t.Fatal(err)
			}
			actions := readLines(t, filepath.Join(manager.Home, "actions"))
			if len(actions) == 0 && test.action == "" {
				return
			}
			if len(actions) != 1 || actions[0] != test.action {
				t.Fatalf("actions = %v, want [%s]", actions, test.action)
			}
		})
	}
}

// A failed first install must not replace packages that already finished.
func TestPkgsrcResumesPartialInstall(t *testing.T) {
	manager := reconcileManager(t, "tool-1", "tool-1", false)
	if err := os.Remove(manager.treeStatePath()); err != nil {
		t.Fatal(err)
	}
	if err := manager.Reconcile(); err != nil {
		t.Fatal(err)
	}
	if actions := readLines(t, filepath.Join(manager.Home, "actions")); len(actions) != 0 {
		t.Fatalf("actions = %v, want none", actions)
	}
}

// Retrying a partial package mutation can damage package state.
func TestPkgsrcDoesNotRetryMutation(t *testing.T) {
	manager := reconcileManager(t, "", "tool-1", true)
	if err := manager.Reconcile(); err == nil {
		t.Fatal("Reconcile() error = nil, want mutation failure")
	}
	actions := readLines(t, filepath.Join(manager.Home, "actions"))
	if len(actions) != 1 {
		t.Fatalf("mutation count = %d, want 1", len(actions))
	}
}

// Command success does not prove the requested version was installed.
func TestPkgsrcVerifiesInstalledVersion(t *testing.T) {
	manager := reconcileManager(t, "", "tool-2", false)
	err := manager.Reconcile()
	if err == nil || !strings.Contains(err.Error(), `installed "tool-2", want exactly "tool-1"`) {
		t.Fatalf("Reconcile() error = %v, want exact version failure", err)
	}
}

// Inventory must report package database failures honestly.
func TestPkgsrcStatusesReturnFailures(t *testing.T) {
	manager := testManager(t)
	writeProbeFixtures(t, manager, "2")
	if _, err := manager.Statuses(); err == nil {
		t.Fatal("Statuses() error = nil, want probe failure")
	}
}

func reconcileManager(t *testing.T, installed, postVersion string, failMutation bool) Manager {
	t.Helper()
	manager := testManager(t)
	writeSourceTree(t, manager)
	if err := manager.writeSourceDigest(); err != nil {
		t.Fatal(err)
	}
	if err := os.MkdirAll(manager.pkgDB(), 0o755); err != nil {
		t.Fatal(err)
	}
	writeFile(t, filepath.Join(manager.Home, "installed"), []byte(installed+"\n"), 0o644)

	bmake := `#!/bin/sh
set -eu
last=""
for arg in "$@"; do last="$arg"; done
case "$*" in
  *show-var*)
    case "$*" in
      *VARNAME=PKGNAME*) printf '%s\n' tool-1 ;;
      *VARNAME=PKGBASE*) printf '%s\n' tool ;;
      *VARNAME=PKG_DBDIR*) printf '%s\n' "$PKG_DBDIR" ;;
    esac
    ;;
  *)
    case "$last" in
    install|replace)
    printf '%s\n' "$last" >> "` + manager.Home + `/actions"
    if [ "` + boolString(failMutation) + `" = true ]; then exit 2; fi
    printf '%s\n' "` + postVersion + `" > "` + manager.Home + `/installed"
    ;;
    esac
    ;;
esac
`
	pkgInfo := `#!/bin/sh
set -eu
installed=$(tr -d '\n' < "` + manager.Home + `/installed")
if [ -z "$installed" ]; then exit 1; fi
printf '%s\n' "$installed"
`
	writeExecutable(t, manager.Bmake(), bmake)
	writeExecutable(t, manager.pkgInfo(), pkgInfo)

	state := treeState{Release: release, ArchiveSHA256: archiveSHA256, Packages: manifest.PkgsrcPackages()}
	data, err := json.Marshal(state)
	if err != nil {
		t.Fatal(err)
	}
	writeFile(t, manager.treeStatePath(), append(data, '\n'), 0o644)
	return manager
}

func testManager(t *testing.T) Manager {
	t.Helper()
	home := t.TempDir()
	return New(home, run.Runner{})
}

func writeSourceTree(t *testing.T, manager Manager) {
	t.Helper()
	writeFile(t, filepath.Join(manager.SourceRoot, "CVS", "Tag"), []byte("Tpkgsrc-"+release+"\n"), 0o644)
	writeFile(t, filepath.Join(manager.SourceRoot, "doc", "CHANGES-pkgsrc-"+release), nil, 0o644)
	writeFile(t, filepath.Join(manager.SourceRoot, "bootstrap", "bootstrap"), []byte("#!/bin/sh\n"), 0o755)
	for _, packagePath := range manifest.PkgsrcPackages() {
		writeFile(t, filepath.Join(manager.SourceRoot, packagePath, "Makefile"), nil, 0o644)
	}
}

func writeProbeFixtures(t *testing.T, manager Manager, exit string) {
	t.Helper()
	packagePath := manifest.PkgsrcPackages()[0]
	if err := os.MkdirAll(filepath.Join(manager.SourceRoot, packagePath), 0o755); err != nil {
		t.Fatal(err)
	}
	writeExecutable(t, manager.Bmake(), `#!/bin/sh
case "$*" in
  *VARNAME=PKGNAME*) printf '%s\n' tool-1 ;;
  *VARNAME=PKGBASE*) printf '%s\n' tool ;;
esac
`)
	writeExecutable(t, manager.pkgInfo(), "#!/bin/sh\nexit "+exit+"\n")
}

func writeExecutable(t *testing.T, path, contents string) {
	t.Helper()
	writeFile(t, path, []byte(contents), 0o755)
}

func writeFile(t *testing.T, path string, contents []byte, mode os.FileMode) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, contents, mode); err != nil {
		t.Fatal(err)
	}
}

func readLines(t *testing.T, path string) []string {
	t.Helper()
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return nil
	}
	if err != nil {
		t.Fatal(err)
	}
	return strings.Fields(string(data))
}

func boolString(value bool) string {
	if value {
		return "true"
	}
	return "false"
}
