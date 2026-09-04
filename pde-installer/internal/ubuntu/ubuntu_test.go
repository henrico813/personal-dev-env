package ubuntu

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"pde-installer/internal/run"
)

func TestPackageSet(t *testing.T) {
	want := []string{
		"bison", "build-essential", "bzip2", "ca-certificates", "curl", "file",
		"fontconfig", "gawk", "git", "gzip", "libevent-dev", "libncurses-dev",
		"make", "patch", "pkg-config", "python3", "tar", "unzip", "xclip",
		"xz-utils", "zsh",
	}
	if strings.Join(packages(), " ") != strings.Join(want, " ") {
		t.Fatalf("packages = %v, want %v", packages(), want)
	}
}

func TestReleaseSupport(t *testing.T) {
	tests := []struct {
		name    string
		release string
		wantErr bool
	}{
		{name: "minimum release", release: "ID=ubuntu\nVERSION_ID=22.04\n"},
		{name: "quoted newer release", release: "ID=ubuntu\nVERSION_ID=\"24.04\"\n"},
		{name: "older Ubuntu", release: "ID=ubuntu\nVERSION_ID=20.04\n", wantErr: true},
		{name: "different distribution", release: "ID=debian\nVERSION_ID=24.04\n", wantErr: true},
		{name: "missing version", release: "ID=ubuntu\n", wantErr: true},
	}
	for _, test := range tests {
		t.Run(test.name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "os-release")
			writeFixture(t, path, test.release, 0o644)
			err := validateRelease(path)
			if (err != nil) != test.wantErr {
				t.Fatalf("validateRelease() error = %v, wantErr %t", err, test.wantErr)
			}
		})
	}
}

func TestReconcileInstallsMissingTogether(t *testing.T) {
	manager, log := fixtureManager(t, map[string]string{"git": "ii", "unzip": "rc"}, false)
	if err := manager.Reconcile(); err != nil {
		t.Fatal(err)
	}
	lines := fixtureLines(t, log)
	if len(lines) != 2 {
		t.Fatalf("sudo calls = %v, want two", lines)
	}
	if lines[0] != "apt-get update" {
		t.Fatalf("update call = %q", lines[0])
	}
	want := "apt-get install -y --no-install-recommends " + strings.Join(missingExcept("git"), " ")
	if lines[1] != want {
		t.Fatalf("install call = %q, want %q", lines[1], want)
	}
	if !strings.Contains(lines[1], " unzip ") {
		t.Fatalf("install call omits unzip: %q", lines[1])
	}
}

func TestReconcileSkipsCurrentPackages(t *testing.T) {
	installed := make(map[string]string, len(packages()))
	for _, name := range packages() {
		installed[name] = "ii"
	}
	manager, log := fixtureManager(t, installed, false)
	if err := manager.Reconcile(); err != nil {
		t.Fatal(err)
	}
	if lines := fixtureLines(t, log); len(lines) != 0 {
		t.Fatalf("sudo calls = %v, want none", lines)
	}
}

func TestDryRunDoesNotMutate(t *testing.T) {
	manager, log := fixtureManager(t, nil, true)
	if err := manager.Reconcile(); err != nil {
		t.Fatal(err)
	}
	if lines := fixtureLines(t, log); len(lines) != 0 {
		t.Fatalf("sudo calls = %v, want none", lines)
	}
}

func TestProbeFailureStopsReconcile(t *testing.T) {
	manager, log := fixtureManager(t, map[string]string{"bison": "fail"}, false)
	if err := manager.Reconcile(); err == nil {
		t.Fatal("Reconcile() error = nil, want probe failure")
	}
	if lines := fixtureLines(t, log); len(lines) != 0 {
		t.Fatalf("sudo calls = %v, want none", lines)
	}
}

func fixtureManager(t *testing.T, installed map[string]string, dryRun bool) (Manager, string) {
	t.Helper()
	root := t.TempDir()
	bin := filepath.Join(root, "bin")
	log := filepath.Join(root, "sudo.log")
	if err := os.MkdirAll(bin, 0o755); err != nil {
		t.Fatal(err)
	}
	var cases strings.Builder
	for name, status := range installed {
		if status == "fail" {
			cases.WriteString(name + ") exit 2 ;;\n")
			continue
		}
		cases.WriteString(name + ") printf '%s' '" + status + "' ;;\n")
	}
	dpkg := "#!/bin/sh\ncase \"$3\" in\n" + cases.String() + "*) exit 1 ;;\nesac\n"
	writeFixture(t, filepath.Join(bin, "dpkg-query"), dpkg, 0o755)
	writeFixture(t, filepath.Join(bin, "sudo"), "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \""+log+"\"\n", 0o755)
	t.Setenv("PATH", bin+string(os.PathListSeparator)+os.Getenv("PATH"))
	release := filepath.Join(root, "os-release")
	writeFixture(t, release, "ID=ubuntu\nVERSION_ID=24.04\n", 0o644)
	manager := New(run.Runner{DryRun: dryRun, Stdout: &strings.Builder{}, Stderr: &strings.Builder{}})
	manager.osReleasePath = release
	return manager, log
}

func missingExcept(current string) []string {
	missing := make([]string, 0, len(packages())-1)
	for _, name := range packages() {
		if name != current {
			missing = append(missing, name)
		}
	}
	return missing
}

func writeFixture(t *testing.T, path, contents string, mode os.FileMode) {
	t.Helper()
	if err := os.WriteFile(path, []byte(contents), mode); err != nil {
		t.Fatal(err)
	}
}

func fixtureLines(t *testing.T, path string) []string {
	t.Helper()
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return nil
	}
	if err != nil {
		t.Fatal(err)
	}
	return strings.Split(strings.TrimSpace(string(data)), "\n")
}
