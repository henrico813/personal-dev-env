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

// The script runs after every apply, so it must link the deployed plugin
// directory at most once and never fail the install when Herdr cannot help.
func TestHerdrLinkScriptNeverFailsInstall(t *testing.T) {
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

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			home := t.TempDir()
			bin := t.TempDir()
			pluginDir := filepath.Join(home, ".local", "share", "pde", "herdr-plugins", "pde-approval")
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
			// /usr/bin and /bin supply the host's jq.
			path := bin + string(os.PathListSeparator) + "/usr/bin:/bin"
			// HERDR_MODE picks the fake's answers; HERDR_MANIFEST is the path it
			// reports for an existing link to this manifest.
			command := exec.Command("sh", script)
			command.Env = append(os.Environ(),
				"HOME="+home,
				"PATH="+path,
				"HERDR_MODE="+tt.herdrState,
				"HERDR_LOG="+logPath,
				"HERDR_MANIFEST="+filepath.Join(pluginDir, "herdr-plugin.toml"),
			)

			output, err := command.CombinedOutput()

			if err != nil {
				t.Fatalf("script failed the install: %v\n%s", err, output)
			}
			wantOutput := strings.ReplaceAll(tt.wantOutput, "PLUGIN", pluginDir)
			if wantOutput == "" && len(output) != 0 || !strings.Contains(string(output), wantOutput) {
				t.Fatalf("output = %q, want substring %q", output, wantOutput)
			}
			calls, err := os.ReadFile(logPath)
			if err != nil && !errors.Is(err, fs.ErrNotExist) {
				t.Fatal(err)
			}
			if unlinked := containsLine(string(calls), "plugin unlink pde.approval"); unlinked != tt.wantUnlink {
				t.Errorf("unlink ran = %v, want %v; calls:\n%s", unlinked, tt.wantUnlink, calls)
			}
			if linked := containsLine(string(calls), "plugin link "+pluginDir); linked != tt.wantLink {
				t.Errorf("link ran = %v, want %v; calls:\n%s", linked, tt.wantLink, calls)
			}
		})
	}
}

// The shared renderer supplies only .chezmoidata.json, and this script needs
// .chezmoi.destDir pointed at the test's home.
func renderHerdrSetup(t *testing.T, home string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(repoRoot(t), "chezmoi", "run_after_link_herdr_approval.sh.tmpl"))
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
      already) printf '{"result":{"plugins":[{"plugin_id":"pde.approval","manifest_path":"%s"}]}}\n' "$HERDR_MANIFEST" ;;
      elsewhere) printf '{"result":{"plugins":[{"plugin_id":"pde.approval","manifest_path":"/other/herdr-plugin.toml"}]}}\n' ;;
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
