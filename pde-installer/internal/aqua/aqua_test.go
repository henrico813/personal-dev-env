package aqua

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"runtime"
	"strings"
	"testing"

	"pde-installer/internal/manifest"
	"pde-installer/internal/profile"
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
	_, status, err := New(home, t.TempDir(), profile.Full, run.Runner{}).Probe()
	if err == nil || status != "" || !strings.Contains(err.Error(), "exit status 9") {
		t.Fatalf("Probe() = _, %q, %v", status, err)
	}
	if _, status := New(home, t.TempDir(), profile.Full, run.Runner{}).Status(); status != "error" {
		t.Fatalf("Status() state = %q", status)
	}
}

// Missing Aqua tools must not be reported as current.
func TestAquaProbeReportsMissingTool(t *testing.T) {
	t.Parallel()
	_, status, err := New(t.TempDir(), t.TempDir(), profile.Full, run.Runner{}).ToolProbe("fd", "v8.3.1")
	if err != nil || status != "missing" {
		t.Fatalf("ToolProbe() = _, %q, %v", status, err)
	}
}

func TestAquaRejectsInvalidProfile(t *testing.T) {
	_, err := New(t.TempDir(), t.TempDir(), profile.Profile("desktop"), run.Runner{}).Reconcile()
	if err == nil || !strings.Contains(err.Error(), `invalid profile "desktop"`) {
		t.Fatalf("Reconcile() error = %v", err)
	}
}

func TestProfileAquaPackages(t *testing.T) {
	for _, selected := range []profile.Profile{profile.Full, profile.Terminal} {
		t.Run(string(selected), func(t *testing.T) {
			configName, _ := selected.AquaFiles()
			configPath := filepath.Join("..", "..", "..", "chezmoi", "dot_config", "aquaproj-aqua", configName)
			configData, err := os.ReadFile(configPath)
			if err != nil {
				t.Fatal(err)
			}
			var packages []string
			for _, line := range strings.Split(string(configData), "\n") {
				line = strings.TrimSpace(line)
				if strings.HasPrefix(line, "- name: ") {
					packages = append(packages, strings.TrimPrefix(line, "- name: "))
				}
			}
			var expectedPackages []string
			for _, item := range manifest.ByOwnerFor(selected, manifest.Aqua) {
				if item.Name == "aqua" || item.Name == "ya" {
					continue
				}
				repository := aquaRepository(t, item.Name)
				expectedPackages = append(expectedPackages, repository+"@"+item.Version)
			}
			if !reflect.DeepEqual(packages, expectedPackages) {
				t.Fatalf("%s packages = %#v, want %#v", configName, packages, expectedPackages)
			}
		})
	}
}

func TestProfileAquaChecksums(t *testing.T) {
	for _, selected := range []profile.Profile{profile.Full, profile.Terminal} {
		t.Run(string(selected), func(t *testing.T) {
			_, checksumsName := selected.AquaFiles()
			checksumsPath := filepath.Join("..", "..", "..", "chezmoi", "dot_config", "aquaproj-aqua", checksumsName)
			ids := readChecksumIDs(t, checksumsPath)
			if !checksumContains(ids, "registries/github_content/github.com/aquaproj/aqua-registry/v4.550.0") {
				t.Fatal("standard registry checksum is missing")
			}
			for _, item := range manifest.ByOwnerFor(selected, manifest.Aqua) {
				if item.Name == "aqua" || item.Name == "ya" || item.Name == "gopls" {
					continue
				}
				repository := aquaRepository(t, item.Name)
				fragment := "github.com/" + repository + "/" + item.Version
				matches := func(architectures ...string) bool {
					for _, id := range ids {
						if !strings.Contains(id, fragment) || !strings.Contains(id, "linux") {
							continue
						}
						for _, architecture := range architectures {
							if strings.Contains(id, architecture) {
								return true
							}
						}
					}
					return false
				}
				if !matches("aarch64", "arm64") || !matches("x86_64", "x64", "amd64") {
					t.Fatalf("Linux checksum artifacts for %s are missing", item.Name)
				}
			}
		})
	}
}

func aquaRepository(t *testing.T, name string) string {
	t.Helper()
	repositories := map[string]string{
		"fd": "sharkdp/fd", "fzf": "junegunn/fzf", "ripgrep": "BurntSushi/ripgrep", "bat": "sharkdp/bat",
		"jq": "jqlang/jq", "chezmoi": "twpayne/chezmoi", "eza": "eza-community/eza", "zoxide": "ajeetdsouza/zoxide",
		"bottom": "ClementTsang/bottom", "yq": "mikefarah/yq", "yazi": "sxyazi/yazi", "gopls": "golang.org/x/tools/gopls",
		"lua-language-server": "LuaLS/lua-language-server",
	}
	repository, ok := repositories[name]
	if !ok {
		t.Fatalf("no expected Aqua package for %s", name)
	}
	return repository
}

func readChecksumIDs(t *testing.T, path string) []string {
	t.Helper()
	var document struct {
		Checksums []struct {
			ID string `json:"id"`
		} `json:"checksums"`
	}
	data, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := json.Unmarshal(data, &document); err != nil {
		t.Fatal(err)
	}
	ids := make([]string, 0, len(document.Checksums))
	for _, checksum := range document.Checksums {
		ids = append(ids, checksum.ID)
	}
	return ids
}

func checksumContains(ids []string, fragment string) bool {
	for _, id := range ids {
		if strings.Contains(id, fragment) {
			return true
		}
	}
	return false
}

func TestYaziProvidesYaExecutable(t *testing.T) {
	candidates := tools(profile.Terminal)
	count := 0
	for _, candidate := range candidates {
		if candidate.name == "ya" {
			count++
		}
	}
	if count != 1 {
		t.Fatalf("terminal currentness candidates contain %d ya entries, want one", count)
	}
	if _, ok := manifest.Find("ya", manifest.Aqua); !ok {
		t.Fatal("ya is missing from the Aqua manifest")
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
		{name: "ya", version: "v25.5.31", path: "github_release/github.com/sxyazi/yazi/v25.5.31/ya", argument: "--version", output: "ya v25.5.31"},
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
			installed, status, err := New(home, t.TempDir(), profile.Full, run.Runner{}).ToolProbe(test.name, test.version)
			if err != nil || status != "current" || !strings.Contains(installed, test.output) {
				t.Fatalf("ToolProbe() = %q, %q, %v", installed, status, err)
			}
		})
	}
}
