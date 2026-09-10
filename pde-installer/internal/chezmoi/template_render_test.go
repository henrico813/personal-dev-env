package chezmoi

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"text/template"
)

func TestProfileTemplatesRender(t *testing.T) {
	tests := []struct {
		profile string
		want    map[string][]string
		haveNot map[string][]string
	}{
		{
			profile: "terminal",
			want: map[string][]string{
				".chezmoiignore.tmpl": {".config/aquaproj-aqua/aqua.yaml", ".config/alacritty", ".config/wezterm", ".config/nvim", ".config/opencode", ".codex", ".agents", ".pi", ".config/nvim/pack/plugins/start/blink.cmp"},
				".chezmoiexternal.toml.tmpl": {".tmux/plugins/tmux-resurrect", "type = \"archive\""},
				"dot_zshrc.tmpl": {"aqua-terminal.yaml", "colored-man-pages", "HISTSIZE=1000000"},
				"dot_tmux.conf.tmpl": {"set -s set-clipboard on", "allow-passthrough on", "@resurrect-processes 'ssh'"},
			},
			haveNot: map[string][]string{
				".chezmoiexternal.toml.tmpl": {"obsidian.nvim", ".pi/agent/settings.json"},
				"dot_zshrc.tmpl": {"keychain --eval", "node{{", "list-npm-globals", "alias vim=", "EDITOR=$(which nvim)", "aqua.yaml"},
				"dot_tmux.conf.tmpl": {"@resurrect-strategy-vim", "@resurrect-strategy-nvim", "@resurrect-processes 'ssh vim nvim'"},
			},
		},
		{
			profile: "full",
			want: map[string][]string{
				".chezmoiignore.tmpl": {".config/aquaproj-aqua/aqua-terminal.yaml", ".config/nvim/pack/plugins/start/blink.cmp"},
				".chezmoiexternal.toml.tmpl": {"obsidian.nvim", ".pi/agent/settings.json", "implement-plan/SKILL.md"},
				"dot_zshrc.tmpl": {"keychain --eval", "node", "list-npm-globals", "alias vim=", "EDITOR=$(which nvim)", "aqua.yaml"},
				"dot_tmux.conf.tmpl": {"@resurrect-strategy-vim", "@resurrect-strategy-nvim", "@resurrect-processes 'ssh vim nvim'", "set -s set-clipboard on"},
			},
			haveNot: map[string][]string{
				".chezmoiignore.tmpl": {".config/alacritty", ".config/wezterm", ".config/opencode"},
				".chezmoiexternal.toml.tmpl": {"PDE_PROFILE must be exactly"},
				"dot_tmux.conf.tmpl": {"@resurrect-processes 'ssh'"},
			},
		},
	}

	for _, test := range tests {
		t.Run(test.profile, func(t *testing.T) {
			for name, want := range test.want {
				text := renderProfileTemplate(t, name, test.profile)
				for _, value := range want {
					if !strings.Contains(text, value) {
						t.Errorf("%s omits %q", name, value)
					}
				}
				for _, value := range test.haveNot[name] {
					if strings.Contains(text, value) {
						t.Errorf("%s contains %q", name, value)
					}
				}
			}
		})
	}
}

func TestProfileTemplateRejectsMissingProfile(t *testing.T) {
	for _, profile := range []string{"", "desktop"} {
		t.Run(profile, func(t *testing.T) {
			if _, err := renderTemplate(t, ".chezmoiignore.tmpl", profile); err == nil {
				t.Fatal("render succeeded")
			}
		})
	}
}

func TestExternalChecksumsMatchSources(t *testing.T) {
	text := renderProfileTemplate(t, ".chezmoiexternal.toml.tmpl", "full")
	if strings.Count(text, "type = ") != strings.Count(text, "checksum.sha256") {
		t.Fatal("external types and checksums differ")
	}
	pattern := regexp.MustCompile(`(?m)^url = 'file://([^']+)'\nchecksum.sha256 = "([^"]+)"`)
	matches := pattern.FindAllStringSubmatch(text, -1)
	if len(matches) == 0 {
		t.Fatal("no local externals rendered")
	}
	for _, match := range matches {
		data, err := os.ReadFile(match[1])
		if err != nil {
			t.Fatal(err)
		}
		hash := sha256.Sum256(data)
		if got := hex.EncodeToString(hash[:]); got != match[2] {
			t.Errorf("checksum for %s = %s, want %s", match[1], got, match[2])
		}
	}
}

func renderProfileTemplate(t *testing.T, name, profile string) string {
	t.Helper()
	text, err := renderTemplate(t, name, profile)
	if err != nil {
		t.Fatal(err)
	}
	return text
}

func renderTemplate(t *testing.T, name, profile string) (string, error) {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(repoRoot(t), "chezmoi", name))
	if err != nil {
		t.Fatal(err)
	}
	functions := template.FuncMap{
		"env": func(key string) string {
			if key == "PDE_PROFILE" {
				return profile
			}
			if key == "PDE_REPO_ROOT" {
				return repoRoot(t)
			}
			return ""
		},
		"fail": func(message string) (string, error) { return "", &templateError{message} },
	}
	tmpl, err := template.New(name).Funcs(functions).Parse(string(data))
	if err != nil {
		return "", err
	}
	var output bytes.Buffer
	if err := tmpl.Execute(&output, nil); err != nil {
		return "", err
	}
	return output.String(), nil
}

type templateError struct{ message string }

func (e *templateError) Error() string { return e.message }

func repoRoot(t *testing.T) string {
	t.Helper()
	root, err := filepath.Abs(filepath.Join("..", "..", ".."))
	if err != nil {
		t.Fatal(err)
	}
	return root
}
