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

func TestIgnoreTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {profile: "terminal", want: []string{".config/aquaproj-aqua/aqua.yaml", ".config/alacritty", ".config/wezterm", ".config/nvim", ".config/opencode", ".codex", ".agents", ".pi", ".config/nvim/pack/plugins/start/blink.cmp"}},
		"full":     {profile: "full", want: []string{".config/aquaproj-aqua/aqua-terminal.yaml", ".config/nvim/pack/plugins/start/blink.cmp"}, omit: []string{".config/alacritty", ".config/wezterm", ".config/opencode"}},
	}
	assertProfileTemplates(t, ".chezmoiignore.tmpl", tests)
}

func TestExternalTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {
			profile: "terminal",
			want:    []string{".tmux/plugins/tmux-resurrect", "type = \"archive\""},
			omit: []string{
				"obsidian.nvim",
				".pi/agent/settings.json",
				".agents/skills/go-development/Apache-2.0.txt",
				".agents/skills/go-development/README.md",
				".agents/skills/go-development/SKILL.md",
				".agents/skills/go-development/examples/composition-original.go",
				".agents/skills/go-development/references/composition.md",
				".agents/skills/go-development/references/uber-go-guide.md",
				".codex/skills/go-development/Apache-2.0.txt",
				".codex/skills/go-development/README.md",
				".codex/skills/go-development/SKILL.md",
				".codex/skills/go-development/examples/composition-original.go",
				".codex/skills/go-development/references/composition.md",
				".codex/skills/go-development/references/uber-go-guide.md",
			},
		},
		"full": {
			profile: "full",
			want: []string{
				"obsidian.nvim",
				".pi/agent/settings.json",
				"implement-plan/SKILL.md",
				".agents/skills/go-development/Apache-2.0.txt",
				".agents/skills/go-development/README.md",
				".agents/skills/go-development/SKILL.md",
				".agents/skills/go-development/examples/composition-original.go",
				".agents/skills/go-development/references/composition.md",
				".agents/skills/go-development/references/uber-go-guide.md",
				".codex/skills/go-development/Apache-2.0.txt",
				".codex/skills/go-development/README.md",
				".codex/skills/go-development/SKILL.md",
				".codex/skills/go-development/examples/composition-original.go",
				".codex/skills/go-development/references/composition.md",
				".codex/skills/go-development/references/uber-go-guide.md",
			},
			omit: []string{"PDE_PROFILE must be exactly"},
		},
	}
	assertProfileTemplates(t, ".chezmoiexternal.toml.tmpl", tests)
}

func TestZshTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {profile: "terminal", want: []string{"aqua-terminal.yaml", "aqua-terminal-checksums.json", "colored-man-pages", "HISTSIZE=1000000"}, omit: []string{"keychain --eval", "node{{", "list-npm-globals", "alias vim=", "EDITOR=$(which nvim)", "/aqua.yaml", "/aqua-checksums.json"}},
		"full":     {profile: "full", want: []string{"keychain --eval", "node", "list-npm-globals", "alias vim=", "EDITOR=$(which nvim)", "/aqua.yaml", "/aqua-checksums.json"}, omit: []string{"aqua-terminal.yaml", "aqua-terminal-checksums.json"}},
	}
	assertProfileTemplates(t, "dot_zshrc.tmpl", tests)
}

func TestTmuxTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {profile: "terminal", want: []string{"set -s set-clipboard on", "allow-passthrough on", "@resurrect-processes 'ssh'"}, omit: []string{"@resurrect-strategy-vim", "@resurrect-strategy-nvim", "@resurrect-processes 'ssh vim nvim'"}},
		"full":     {profile: "full", want: []string{"@resurrect-strategy-vim", "@resurrect-strategy-nvim", "@resurrect-processes 'ssh vim nvim'", "set -s set-clipboard on"}, omit: []string{"@resurrect-processes 'ssh'"}},
	}
	assertProfileTemplates(t, "dot_tmux.conf.tmpl", tests)
}

func TestTemplatesRejectInvalidProfiles(t *testing.T) {
	for name, selected := range map[string]string{"missing": "", "unknown": "desktop"} {
		t.Run(name, func(t *testing.T) {
			if _, err := renderTemplate(t, ".chezmoiignore.tmpl", selected); err == nil {
				t.Fatal("render succeeded")
			}
		})
	}
}

func assertProfileTemplates(t *testing.T, templateName string, tests map[string]struct {
	profile string
	want    []string
	omit    []string
}) {
	t.Helper()
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			text := renderProfileTemplate(t, templateName, test.profile)
			for _, value := range test.want {
				if !strings.Contains(text, value) {
					t.Errorf("%s omits %q", templateName, value)
				}
			}
			for _, value := range test.omit {
				if strings.Contains(text, value) {
					t.Errorf("%s contains %q", templateName, value)
				}
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
