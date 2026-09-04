package aqua

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"pde-installer/internal/run"
)

// A broken executable needs repair instead of another installation.
func TestAquaProbeReturnsCommandErrors(t *testing.T) {
	t.Parallel()
	home := t.TempDir()
	binary := filepath.Join(home, ".local", "share", "aquaproj-aqua", "bin", "aqua")
	if err := os.MkdirAll(filepath.Dir(binary), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(binary, []byte("#!/bin/sh\nexit 9\n"), 0o755); err != nil {
		t.Fatal(err)
	}
	_, status, err := New(home, t.TempDir(), run.Runner{}).Probe()
	if err == nil || status != "" || !strings.Contains(err.Error(), "exit status 9") {
		t.Fatalf("Probe() = _, %q, %v", status, err)
	}
	if _, status := New(home, t.TempDir(), run.Runner{}).Status(); status != "error" {
		t.Fatalf("Status() state = %q", status)
	}
}

// Missing Aqua tools must not be reported as current.
func TestAquaProbeReportsMissingTool(t *testing.T) {
	t.Parallel()
	_, status, err := New(t.TempDir(), t.TempDir(), run.Runner{}).ToolProbe("fd", "v8.3.1")
	if err != nil || status != "missing" {
		t.Fatalf("ToolProbe() = _, %q, %v", status, err)
	}
}

func TestToolProbeUsesPackageBinaries(t *testing.T) {
	t.Parallel()
	for _, test := range []struct {
		name, version, path, argument, output string
	}{
		{name: "gopls", version: "v0.23.0", path: "golang.org/x/tools/gopls/v0.23.0/bin/gopls", argument: "version", output: "golang.org/x/tools/gopls v0.23.0"},
		{name: "jq", version: "jq-1.7.1", path: "github_release/github.com/jqlang/jq/jq-1.7.1/jq-linux-" + runtime.GOARCH + "/jq-linux-" + runtime.GOARCH, argument: "--version", output: "jq-1.7.1"},
		{name: "yq", version: "v4.53.3", path: "github_release/github.com/mikefarah/yq/v4.53.3/yq_linux_" + runtime.GOARCH + "/yq_linux_" + runtime.GOARCH, argument: "--version", output: "yq version v4.53.3"},
	} {
		t.Run(test.name, func(t *testing.T) {
			home := t.TempDir()
			binary := filepath.Join(home, ".local", "share", "aquaproj-aqua", "pkgs", filepath.FromSlash(test.path))
			if err := os.MkdirAll(filepath.Dir(binary), 0o755); err != nil {
				t.Fatal(err)
			}
			script := "#!/bin/sh\n[ \"$1\" = \"" + test.argument + "\" ] || exit 9\nprintf '%s\\n' '" + test.output + "'\n"
			if err := os.WriteFile(binary, []byte(script), 0o755); err != nil {
				t.Fatal(err)
			}
			installed, status, err := New(home, t.TempDir(), run.Runner{}).ToolProbe(test.name, test.version)
			if err != nil || status != "current" || !strings.Contains(installed, test.output) {
				t.Fatalf("ToolProbe() = %q, %q, %v", installed, status, err)
			}
		})
	}
}
