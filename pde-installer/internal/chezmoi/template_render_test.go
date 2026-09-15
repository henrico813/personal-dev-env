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
		"terminal": {profile: "terminal", want: []string{".config/aquaproj-aqua/aqua.yaml", ".config/alacritty", ".config/wezterm", ".config/nvim", ".config/opencode", ".config/systemd/user/opencode-web.service", ".config/systemd/user/opencode-web-health.service", ".config/systemd/user/opencode-web-health.timer", ".codex", ".agents", ".pi", ".config/nvim/pack/plugins/start/blink.cmp"}},
		"full":     {profile: "full", want: []string{".config/aquaproj-aqua/aqua-terminal.yaml", ".config/nvim/pack/plugins/start/blink.cmp"}, omit: []string{".config/alacritty", ".config/wezterm", ".config/opencode", ".config/systemd/user/opencode-web.service", ".config/systemd/user/opencode-web-health.service", ".config/systemd/user/opencode-web-health.timer"}},
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
				".agents/skills/obsidian-zettel/SKILL.md",
				".codex/skills/obsidian-zettel/SKILL.md",
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
				// Rust Agent Skills package.
				".agents/skills/rust-development/SKILL.md",
				".agents/skills/rust-development/examples/.gitignore",
				".agents/skills/rust-development/examples/Cargo.lock",
				".agents/skills/rust-development/examples/Cargo.toml",
				".agents/skills/rust-development/examples/README.md",
				".agents/skills/rust-development/examples/src/buffered_output.rs",
				".agents/skills/rust-development/examples/src/device.rs",
				".agents/skills/rust-development/examples/src/foreign.rs",
				".agents/skills/rust-development/examples/src/identity.rs",
				".agents/skills/rust-development/examples/src/lib.rs",
				".agents/skills/rust-development/examples/src/main.rs",
				".agents/skills/rust-development/examples/src/report.rs",
				".agents/skills/rust-development/examples/src/retry.rs",
				".agents/skills/rust-development/examples/src/threaded.rs",
				".agents/skills/rust-development/examples/tests/cli.rs",
				".agents/skills/rust-development/references/api-design.md",
				".agents/skills/rust-development/references/cargo.md",
				".agents/skills/rust-development/references/concurrency.md",
				".agents/skills/rust-development/references/design-choices.md",
				".agents/skills/rust-development/references/documentation.md",
				".agents/skills/rust-development/references/errors-resources.md",
				".agents/skills/rust-development/references/evaluation.md",
				".agents/skills/rust-development/references/examples.md",
				".agents/skills/rust-development/references/ownership.md",
				".agents/skills/rust-development/references/sources.md",
				".agents/skills/rust-development/references/testing.md",
				".agents/skills/rust-development/references/unsafe.md",
				// Rust Codex package.
				".codex/skills/rust-development/SKILL.md",
				".codex/skills/rust-development/examples/.gitignore",
				".codex/skills/rust-development/examples/Cargo.lock",
				".codex/skills/rust-development/examples/Cargo.toml",
				".codex/skills/rust-development/examples/README.md",
				".codex/skills/rust-development/examples/src/buffered_output.rs",
				".codex/skills/rust-development/examples/src/device.rs",
				".codex/skills/rust-development/examples/src/foreign.rs",
				".codex/skills/rust-development/examples/src/identity.rs",
				".codex/skills/rust-development/examples/src/lib.rs",
				".codex/skills/rust-development/examples/src/main.rs",
				".codex/skills/rust-development/examples/src/report.rs",
				".codex/skills/rust-development/examples/src/retry.rs",
				".codex/skills/rust-development/examples/src/threaded.rs",
				".codex/skills/rust-development/examples/tests/cli.rs",
				".codex/skills/rust-development/references/api-design.md",
				".codex/skills/rust-development/references/cargo.md",
				".codex/skills/rust-development/references/concurrency.md",
				".codex/skills/rust-development/references/design-choices.md",
				".codex/skills/rust-development/references/documentation.md",
				".codex/skills/rust-development/references/errors-resources.md",
				".codex/skills/rust-development/references/evaluation.md",
				".codex/skills/rust-development/references/examples.md",
				".codex/skills/rust-development/references/ownership.md",
				".codex/skills/rust-development/references/sources.md",
				".codex/skills/rust-development/references/testing.md",
				".codex/skills/rust-development/references/unsafe.md",
			},
		},
		"full": {
			profile: "full",
			want: []string{
				"obsidian.nvim",
				".pi/agent/settings.json",
				"implement-plan/SKILL.md",
				".agents/skills/obsidian-zettel/SKILL.md",
				".codex/skills/obsidian-zettel/SKILL.md",
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
				// Rust Agent Skills package.
				".agents/skills/rust-development/SKILL.md",
				".agents/skills/rust-development/examples/.gitignore",
				".agents/skills/rust-development/examples/Cargo.lock",
				".agents/skills/rust-development/examples/Cargo.toml",
				".agents/skills/rust-development/examples/README.md",
				".agents/skills/rust-development/examples/src/buffered_output.rs",
				".agents/skills/rust-development/examples/src/device.rs",
				".agents/skills/rust-development/examples/src/foreign.rs",
				".agents/skills/rust-development/examples/src/identity.rs",
				".agents/skills/rust-development/examples/src/lib.rs",
				".agents/skills/rust-development/examples/src/main.rs",
				".agents/skills/rust-development/examples/src/report.rs",
				".agents/skills/rust-development/examples/src/retry.rs",
				".agents/skills/rust-development/examples/src/threaded.rs",
				".agents/skills/rust-development/examples/tests/cli.rs",
				".agents/skills/rust-development/references/api-design.md",
				".agents/skills/rust-development/references/cargo.md",
				".agents/skills/rust-development/references/concurrency.md",
				".agents/skills/rust-development/references/design-choices.md",
				".agents/skills/rust-development/references/documentation.md",
				".agents/skills/rust-development/references/errors-resources.md",
				".agents/skills/rust-development/references/evaluation.md",
				".agents/skills/rust-development/references/examples.md",
				".agents/skills/rust-development/references/ownership.md",
				".agents/skills/rust-development/references/sources.md",
				".agents/skills/rust-development/references/testing.md",
				".agents/skills/rust-development/references/unsafe.md",
				// Rust Codex package.
				".codex/skills/rust-development/SKILL.md",
				".codex/skills/rust-development/examples/.gitignore",
				".codex/skills/rust-development/examples/Cargo.lock",
				".codex/skills/rust-development/examples/Cargo.toml",
				".codex/skills/rust-development/examples/README.md",
				".codex/skills/rust-development/examples/src/buffered_output.rs",
				".codex/skills/rust-development/examples/src/device.rs",
				".codex/skills/rust-development/examples/src/foreign.rs",
				".codex/skills/rust-development/examples/src/identity.rs",
				".codex/skills/rust-development/examples/src/lib.rs",
				".codex/skills/rust-development/examples/src/main.rs",
				".codex/skills/rust-development/examples/src/report.rs",
				".codex/skills/rust-development/examples/src/retry.rs",
				".codex/skills/rust-development/examples/src/threaded.rs",
				".codex/skills/rust-development/examples/tests/cli.rs",
				".codex/skills/rust-development/references/api-design.md",
				".codex/skills/rust-development/references/cargo.md",
				".codex/skills/rust-development/references/concurrency.md",
				".codex/skills/rust-development/references/design-choices.md",
				".codex/skills/rust-development/references/documentation.md",
				".codex/skills/rust-development/references/errors-resources.md",
				".codex/skills/rust-development/references/evaluation.md",
				".codex/skills/rust-development/references/examples.md",
				".codex/skills/rust-development/references/ownership.md",
				".codex/skills/rust-development/references/sources.md",
				".codex/skills/rust-development/references/testing.md",
				".codex/skills/rust-development/references/unsafe.md",
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
		"terminal": {profile: "terminal", want: []string{"aqua-terminal.yaml", "aqua-terminal-checksums.json", "colored-man-pages", "HISTSIZE=1000000"}, omit: []string{"keychain --eval", "node{{", "list-npm-globals", "alias vim=", "oca()", "EDITOR=$(which nvim)", "/aqua.yaml", "/aqua-checksums.json"}},
		"full":     {profile: "full", want: []string{"keychain --eval", "node", "list-npm-globals", "alias vim=", "oca()", "command git rev-parse --show-toplevel", "command curl --fail --silent --max-time 2", "command systemctl --user restart opencode-web.service", "command opencode attach", "--dir \"$dir\"", "\"$@\"", "EDITOR=$(which nvim)", "/aqua.yaml", "/aqua-checksums.json"}, omit: []string{"aqua-terminal.yaml", "aqua-terminal-checksums.json"}},
	}
	assertProfileTemplates(t, "dot_zshrc.tmpl", tests)
}

func TestOpenCodeSetupScriptProfiles(t *testing.T) {
	full := renderProfileTemplate(t, "run_after_configure_opencode_web.sh.tmpl", "full")
	if !strings.Contains(full, "systemctl --user enable --now opencode-web.service") {
		t.Fatalf("full setup script = %q", full)
	}
	terminal := renderProfileTemplate(t, "run_after_configure_opencode_web.sh.tmpl", "terminal")
	if strings.Contains(terminal, "systemctl --user") {
		t.Fatalf("terminal setup script = %q", terminal)
	}
}

func TestOpenCodeSetupScriptIsExecutable(t *testing.T) {
	info, err := os.Stat(filepath.Join(repoRoot(t), "chezmoi", "run_after_configure_opencode_web.sh.tmpl"))
	if err != nil {
		t.Fatal(err)
	}
	if info.Mode()&0o111 == 0 {
		t.Fatalf("setup script mode = %v", info.Mode())
	}
}

func TestOpenCodeServiceUsesManagedPath(t *testing.T) {
	data, err := os.ReadFile(filepath.Join(repoRoot(t), "chezmoi", "dot_config", "systemd", "user", "opencode-web.service"))
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(string(data), "Environment=PATH=%h/.local/bin:") {
		t.Fatalf("service does not include the managed launcher path: %q", data)
	}
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
