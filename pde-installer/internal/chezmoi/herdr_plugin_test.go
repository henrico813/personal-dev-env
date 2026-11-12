package chezmoi

import (
	"bytes"
	"errors"
	"io/fs"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"
	"text/template"

	"pde-installer/internal/testfile"
)

// The script runs after every apply, so it must link each deployed plugin
// directory at most once and never fail the install when Herdr cannot help.
// It runs itself once for each of two plugins, so each case checks both.
func TestHerdrLinkScriptNeverFailsInstall(t *testing.T) {
	plugins := []struct{ id, dir string }{
		{"pde.approval", "pde-approval"},
		{"pde.comment-review", "glimmer"},
	}
	tests := []struct {
		name string
		// The fake herdr's state is "already" (linked to this manifest), "older"
		// (linked to it at an older version), "elsewhere" (linked to another
		// manifest), "missing" (not linked), "unavailable" (server stopped),
		// or "link-fail" (link refused).
		herdrState string
		herdrIn    string // "path", or "home" for only the destination's .local/bin.
		wantOutput string
		wantUnlink bool // Whether the old link is removed with herdr plugin unlink.
		wantLink   bool // Whether herdr plugin link runs for the deployed directory.
	}{
		{
			name:       "already linked",
			herdrState: "already",
			herdrIn:    "path",
		},
		{
			name:       "linked at an older version",
			herdrState: "older",
			herdrIn:    "path",
			wantUnlink: true,
			wantLink:   true,
		},
		{
			name:       "linked elsewhere",
			herdrState: "elsewhere",
			herdrIn:    "path",
			wantUnlink: true,
			wantLink:   true,
		},
		{
			name:       "missing",
			herdrState: "missing",
			herdrIn:    "path",
			wantLink:   true,
		},
		{
			name:       "herdr only in home bin",
			herdrState: "missing",
			herdrIn:    "home",
			wantLink:   true,
		},
		{
			name:       "herdr unavailable",
			herdrState: "unavailable",
			herdrIn:    "path",
			wantOutput: "Herdr is unavailable; link later with: herdr plugin link PLUGIN",
		},
		{
			name:       "link failure",
			herdrState: "link-fail",
			herdrIn:    "path",
			wantOutput: "Herdr plugin link failed:",
			wantLink:   true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			home := t.TempDir()
			pluginsDir := filepath.Join(home, ".local", "share", "pde", "herdr-plugins")

			output, calls := runHerdrSetup(
				t,
				home,
				tt.herdrState,
				tt.herdrIn,
			)

			if tt.wantOutput == "" && len(output) != 0 {
				t.Fatalf("output = %q, want none", output)
			}
			for _, plugin := range plugins {
				pluginDir := filepath.Join(pluginsDir, plugin.dir)
				wantOutput := strings.ReplaceAll(tt.wantOutput, "PLUGIN", pluginDir)
				if !strings.Contains(output, wantOutput) {
					t.Errorf("output = %q, want substring %q", output, wantOutput)
				}
				if unlinked := containsLine(calls, "plugin unlink "+plugin.id); unlinked != tt.wantUnlink {
					t.Errorf("unlink %s ran = %v, want %v; calls:\n%s", plugin.id, unlinked, tt.wantUnlink, calls)
				}
				if linked := containsLine(calls, "plugin link "+pluginDir); linked != tt.wantLink {
					t.Errorf("link %s ran = %v, want %v; calls:\n%s", plugin.id, linked, tt.wantLink, calls)
				}
			}
		})
	}
}

// A Herdr server started before an install kept running without the new
// Ctrl+b e key, so the key opened Herdr's scrollback editor instead of the
// comment review.
func TestHerdrScriptReloadsRunningServerConfig(t *testing.T) {
	home := t.TempDir()

	_, calls := runHerdrSetup(
		t,
		home,
		"already",
		"home",
	)

	if !containsLine(calls, "server reload-config") {
		t.Errorf("herdr server reload-config did not run; calls:\n%s", calls)
	}
}

// When the Herdr server's PATH lacks ~/.local/bin, a key that runs a bare
// `hub go` does nothing, because neither hub nor the herdr and card it runs
// are found.
func TestHerdrShellKeysFindInstalledTools(t *testing.T) {
	home := t.TempDir()
	keys := readHerdrShellKeys(t)
	if len(keys) == 0 {
		t.Fatal("found no shell keys in the Herdr config")
	}

	for _, key := range keys {
		t.Run(key.key, func(t *testing.T) {
			program := findShellKeyProgram(key.command)
			if program == "" {
				t.Fatalf("shell key %s has no command", key.key)
			}
			tool := filepath.Join(
				home,
				".local",
				"bin",
				filepath.Base(program),
			)
			// The fake counts as run only when the tools it calls in turn,
			// such as herdr and card, are on PATH too.
			testfile.WriteExecutable(
				t,
				tool,
				"#!/bin/sh\ncase \":$PATH:\" in *\":$HOME/.local/bin:\"*) : >\"$0.ran\" ;; esac\n",
			)
			// Herdr runs `sh -lc`, but the test leaves out -l so the host's
			// /etc/profile cannot add ~/.local/bin and hide a bare name.
			command := exec.Command(
				"/bin/sh",
				"-c",
				key.command,
			)
			command.Env = buildServerEnvironment(home)

			output, err := command.CombinedOutput()

			if err != nil {
				t.Fatalf(
					"command failed without ~/.local/bin on PATH: %v\n%s",
					err,
					output,
				)
			}
			if _, err := os.Stat(tool + ".ran"); err != nil {
				t.Errorf(
					"%s did not run with ~/.local/bin on its PATH: %v",
					tool,
					err,
				)
			}
		})
	}
}

// Runs the script for home against a fake herdr in herdrState, found as
// herdrIn says, and returns its output and the herdr calls it made. Fails
// the test when the script exits nonzero, which would fail the install.
func runHerdrSetup(
	t *testing.T,
	home string,
	herdrState string,
	herdrIn string,
) (output, calls string) {
	t.Helper()
	// The installer runs the script with aqua's bin, where it installs jq,
	// first on PATH, so take jq from the test's PATH the same way.
	jq, err := exec.LookPath("jq")
	if err != nil {
		t.Skip("jq is not on PATH; the installer provides it through aqua, so install first")
	}
	bin := t.TempDir()
	pluginsDir := filepath.Join(
		home,
		".local",
		"share",
		"pde",
		"herdr-plugins",
	)
	// The fake herdr's already state reports 1.0.0, the version written here.
	for _, dir := range []string{"pde-approval", "glimmer"} {
		manifest := filepath.Join(pluginsDir, dir, "herdr-plugin.toml")
		if err := os.MkdirAll(filepath.Dir(manifest), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(
			manifest,
			[]byte("version = \"1.0.0\"\n"),
			0o644,
		); err != nil {
			t.Fatal(err)
		}
	}
	// The fake herdr logs each call here, so the test can see what ran.
	logPath := filepath.Join(t.TempDir(), "herdr.log")
	// Put the fake herdr on PATH or only in the home's .local/bin.
	switch herdrIn {
	case "path":
		testfile.WriteExecutable(
			t,
			filepath.Join(bin, "herdr"),
			fakeHerdr,
		)
	case "home":
		testfile.WriteExecutable(
			t,
			filepath.Join(home, ".local", "bin", "herdr"),
			fakeHerdr,
		)
	}
	// Render the template for this home, as chezmoi would.
	script := filepath.Join(t.TempDir(), "link-herdr.sh")
	err = os.WriteFile(
		script,
		[]byte(renderHerdrSetup(t, home)),
		0o700,
	)
	if err != nil {
		t.Fatal(err)
	}
	// jq's folder supplies jq, and /usr/bin and /bin the shell tools.
	path := strings.Join(
		[]string{
			bin,
			filepath.Dir(jq),
			"/usr/bin",
			"/bin",
		},
		string(os.PathListSeparator),
	)
	// HERDR_MODE picks the fake's answers; HERDR_PLUGINS is the folder
	// whose manifests it reports as already linked.
	command := exec.Command("sh", script)
	command.Env = append(os.Environ(),
		"HOME="+home,
		"PATH="+path,
		"HERDR_MODE="+herdrState,
		"HERDR_LOG="+logPath,
		"HERDR_PLUGINS="+pluginsDir,
	)

	out, err := command.CombinedOutput()
	if err != nil {
		t.Fatalf("script failed the install: %v\n%s", err, out)
	}
	logged, err := os.ReadFile(logPath)
	if err != nil && !errors.Is(err, fs.ErrNotExist) {
		t.Fatal(err)
	}
	return string(out), string(logged)
}

// The shared renderer supplies only .chezmoidata.json, and this script needs
// .chezmoi.destDir pointed at the test's home.
func renderHerdrSetup(t *testing.T, home string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(repoRoot(t), "chezmoi", "run_after_link_herdr_plugins.sh.tmpl"))
	if err != nil {
		t.Fatal(err)
	}
	tmpl, err := template.New("herdr setup").Parse(string(data))
	if err != nil {
		t.Fatal(err)
	}
	var output bytes.Buffer
	if err := tmpl.Execute(&output, map[string]any{"chezmoi": map[string]string{"destDir": home}}); err != nil {
		t.Fatal(err)
	}
	return output.String()
}

// One shell key in the managed Herdr config.
type herdrShellKey struct {
	key     string
	command string
}

// Returns each shell key in the managed Herdr config. Each one is expected
// to start a tool installed in ~/.local/bin.
func readHerdrShellKeys(t *testing.T) []herdrShellKey {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(
		repoRoot(t),
		"chezmoi",
		"dot_config",
		"herdr",
		"config.toml",
	))
	if err != nil {
		t.Fatal(err)
	}
	var keys []herdrShellKey
	for _, block := range strings.Split(string(data), "[[keys.command]]")[1:] {
		block, _, _ = strings.Cut(block, "\n[")
		values := map[string]string{}
		for _, line := range strings.Split(block, "\n") {
			if key, value, found := strings.Cut(line, " = "); found {
				values[key] = strings.Trim(value, `"'`)
			}
		}
		if values["type"] == "shell" {
			keys = append(keys, herdrShellKey{
				key:     values["key"],
				command: values["command"],
			})
		}
	}
	return keys
}

// Returns the program a shell key starts, the first word that does not set
// a variable.
func findShellKeyProgram(command string) string {
	for _, word := range strings.Fields(command) {
		if !strings.Contains(word, "=") {
			return word
		}
	}
	return ""
}

// Returns this test's environment with the test home as HOME and ZDOTDIR
// and only the system folders on PATH, like a Herdr server started over ssh.
func buildServerEnvironment(home string) []string {
	var environment []string
	for _, value := range os.Environ() {
		if strings.HasPrefix(value, "HOME=") ||
			strings.HasPrefix(value, "ZDOTDIR=") ||
			strings.HasPrefix(value, "PATH=") {
			continue
		}
		environment = append(environment, value)
	}
	return append(environment,
		"HOME="+home,
		"ZDOTDIR="+home,
		"PATH=/usr/bin:/bin",
	)
}

const fakeHerdr = `#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$HERDR_LOG"
case "$1 ${2:-}" in
  status\ server)
    if [ "$HERDR_MODE" = unavailable ]; then
      echo 'server unavailable' >&2
      exit 1
    fi
    ;;
  plugin\ list)
    case "$HERDR_MODE" in
      already) printf '{"result":{"plugins":[{"plugin_id":"pde.approval","version":"1.0.0","manifest_path":"%s"},{"plugin_id":"pde.comment-review","version":"1.0.0","manifest_path":"%s"}]}}\n' \
        "$HERDR_PLUGINS/pde-approval/herdr-plugin.toml" "$HERDR_PLUGINS/glimmer/herdr-plugin.toml" ;;
      older) printf '{"result":{"plugins":[{"plugin_id":"pde.approval","version":"0.9.0","manifest_path":"%s"},{"plugin_id":"pde.comment-review","version":"0.9.0","manifest_path":"%s"}]}}\n' \
        "$HERDR_PLUGINS/pde-approval/herdr-plugin.toml" "$HERDR_PLUGINS/glimmer/herdr-plugin.toml" ;;
      elsewhere) printf '{"result":{"plugins":[{"plugin_id":"pde.approval","manifest_path":"/a/herdr-plugin.toml"},{"plugin_id":"pde.comment-review","manifest_path":"/b/herdr-plugin.toml"}]}}\n' ;;
      *) printf '{"result":{"plugins":[]}}\n' ;;
    esac
    ;;
  plugin\ link)
    if [ "$HERDR_MODE" = link-fail ]; then
      echo 'link refused' >&2
      exit 1
    fi
    ;;
esac
`
