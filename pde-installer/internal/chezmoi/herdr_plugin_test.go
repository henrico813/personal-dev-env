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
)

// The script runs after every apply, so it must link each deployed plugin
// directory at most once and never fail the install when Herdr cannot help.
// It runs itself once for each of two plugins, so each case checks both.
func TestHerdrLinkScriptNeverFailsInstall(t *testing.T) {
	plugins := []struct{ id, dir string }{
		{"pde.approval", "pde-approval"},
		{"pde.comment-review", "pde-comment-review"},
	}
	tests := []struct {
		name string
		// The fakeHerdr state: "already" (linked to this manifest), "elsewhere"
		// (linked to another manifest), "missing" (not linked), "unavailable"
		// (server stopped), or "link-fail" (link refused).
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

	// The installer runs the script with aqua's bin, where it installs jq,
	// first on PATH, so take jq from the test's PATH the same way.
	jq, err := exec.LookPath("jq")
	if err != nil {
		t.Skip("jq is not on PATH; the installer provides it through aqua, so install first")
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			home := t.TempDir()
			bin := t.TempDir()
			pluginsDir := filepath.Join(home, ".local", "share", "pde", "herdr-plugins")
			// The fake herdr logs each call here, so the test can see what ran.
			logPath := filepath.Join(t.TempDir(), "herdr.log")
			// Put the fake herdr on PATH or only in the home's .local/bin.
			switch tt.herdrIn {
			case "path":
				writeExecutable(t, filepath.Join(bin, "herdr"), fakeHerdr)
			case "home":
				writeExecutable(t, filepath.Join(home, ".local", "bin", "herdr"), fakeHerdr)
			}
			// Render the template for this home, as chezmoi would.
			script := filepath.Join(t.TempDir(), "link-herdr.sh")
			if err := os.WriteFile(script, []byte(renderHerdrSetup(t, home)), 0o700); err != nil {
				t.Fatal(err)
			}
			// jq's folder supplies jq, and /usr/bin and /bin the shell tools.
			path := strings.Join([]string{bin, filepath.Dir(jq), "/usr/bin", "/bin"}, string(os.PathListSeparator))
			// HERDR_MODE picks the fake's answers; HERDR_PLUGINS is the folder
			// whose manifests it reports as already linked.
			command := exec.Command("sh", script)
			command.Env = append(os.Environ(),
				"HOME="+home,
				"PATH="+path,
				"HERDR_MODE="+tt.herdrState,
				"HERDR_LOG="+logPath,
				"HERDR_PLUGINS="+pluginsDir,
			)

			output, err := command.CombinedOutput()

			if err != nil {
				t.Fatalf("script failed the install: %v\n%s", err, output)
			}
			if tt.wantOutput == "" && len(output) != 0 {
				t.Fatalf("output = %q, want none", output)
			}
			calls, err := os.ReadFile(logPath)
			if err != nil && !errors.Is(err, fs.ErrNotExist) {
				t.Fatal(err)
			}
			for _, plugin := range plugins {
				pluginDir := filepath.Join(pluginsDir, plugin.dir)
				wantOutput := strings.ReplaceAll(tt.wantOutput, "PLUGIN", pluginDir)
				if !strings.Contains(string(output), wantOutput) {
					t.Errorf("output = %q, want substring %q", output, wantOutput)
				}
				if unlinked := containsLine(string(calls), "plugin unlink "+plugin.id); unlinked != tt.wantUnlink {
					t.Errorf("unlink %s ran = %v, want %v; calls:\n%s", plugin.id, unlinked, tt.wantUnlink, calls)
				}
				if linked := containsLine(string(calls), "plugin link "+pluginDir); linked != tt.wantLink {
					t.Errorf("link %s ran = %v, want %v; calls:\n%s", plugin.id, linked, tt.wantLink, calls)
				}
			}
		})
	}
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
      already) printf '{"result":{"plugins":[{"plugin_id":"pde.approval","manifest_path":"%s"},{"plugin_id":"pde.comment-review","manifest_path":"%s"}]}}\n' \
        "$HERDR_PLUGINS/pde-approval/herdr-plugin.toml" "$HERDR_PLUGINS/pde-comment-review/herdr-plugin.toml" ;;
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
