package chezmoi

import (
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"strings"
	"testing"
	"text/template"

	"pde-installer/internal/colorprofile"
)

func TestIgnoreTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {
			profile: "terminal",
			want: []string{
				".config/aquaproj-aqua/aqua.yaml",
				".config/aquaproj-aqua/aqua-checksums.json",
				".config/alacritty",
				".config/wezterm",
				".config/nvim",
				".config/nvim/**",
				".config/opencode",
				".config/herdr",
				".codex",
				".agents",
				".pi",
				".config/nvim/pack/plugins/start/blink.cmp",
				".config/nvim/pack/plugins/start/opencode-inline.nvim",
				".config/nvim/pack/plugins/start/opencode-inline.nvim/**",
			},
		},
		"full": {
			profile: "full",
			want: []string{
				".config/aquaproj-aqua/aqua-terminal.yaml",
				".config/aquaproj-aqua/aqua-terminal-checksums.json",
				".config/nvim/pack/plugins/start/blink.cmp",
				".config/nvim/pack/plugins/start/opencode-inline.nvim",
				".config/nvim/pack/plugins/start/opencode-inline.nvim/**",
			},
			omit: []string{
				".config/alacritty",
				".config/wezterm",
				".config/nvim",
				".config/nvim/**",
				".config/opencode",
				".config/herdr",
			},
		},
	}
	assertProfileTemplateLines(t, ".chezmoiignore.tmpl", tests)
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
				"copilot.lua",
				".config/opencode/AGENTS.md",
				".agents/skills/code-documentation/SKILL.md",
			},
		},
		"full": {
			profile: "full",
			want: []string{
				"obsidian.nvim",
				"copilot.lua",
				".config/opencode/AGENTS.md",
				".agents/skills/code-documentation/SKILL.md",
			},
		},
	}
	assertProfileTemplates(t, ".chezmoiexternal.toml.tmpl", tests)
}

func TestSharedWorkflowMappings(t *testing.T) {
	text := renderProfileTemplate(t, ".chezmoiexternal.toml.tmpl", "full")
	tests := []struct {
		target string
		source string
	}{
		{target: ".config/opencode/AGENTS.md", source: "ai/AGENTS.md"},
		{target: ".codex/AGENTS.md", source: "ai/AGENTS.md"},
		{target: ".pi/agent/AGENTS.md", source: "ai/AGENTS.md"},
		{target: ".agents/skills/code-documentation/SKILL.md", source: "ai/skills/code-documentation/SKILL.md"},
		{target: ".codex/skills/code-documentation/SKILL.md", source: "ai/skills/code-documentation/SKILL.md"},
	}

	for _, tt := range tests {
		t.Run(tt.target, func(t *testing.T) {
			mapping := fmt.Sprintf(
				"[%q]\ntype = \"file\"\nurl = 'file://%s/%s'",
				tt.target,
				repoRoot(t),
				tt.source,
			)
			if count := strings.Count(text, mapping); count != 1 {
				t.Fatalf("mapping %s count = %d, want 1", tt.target, count)
			}
		})
	}
}

func TestGitConfigEnablesDelta(t *testing.T) {
	path := filepath.Join(repoRoot(t), "chezmoi", "dot_config", "pde", "gitconfig")
	tests := map[string]string{
		"core.pager":             "delta",
		"interactive.diffFilter": "delta --color-only",
		"delta.navigate":         "true",
		"delta.line-numbers":     "true",
		"merge.conflictStyle":    "zdiff3",
	}
	for key, want := range tests {
		t.Run(key, func(t *testing.T) {
			command := exec.Command("git", "config", "--file", path, "--get", key)
			output, err := command.CombinedOutput()
			if err != nil {
				t.Fatalf("git config failed: %v\n%s", err, output)
			}
			if got := strings.TrimSpace(string(output)); got != want {
				t.Fatalf("%s = %q, want %q", key, got, want)
			}
		})
	}
}

func TestZshTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {
			profile: "terminal",
			want: []string{
				"aqua-terminal.yaml",
				"aqua-terminal-checksums.json",
				"colored-man-pages",
				"HISTSIZE=1000000",
				"Usage: tm [hub|pocket|dash] [directory]",
				"scope=\"tm-${slug}-${hash[1,8]}\"",
				"herdr_bin=\"$(command -v herdr)\"",
				"tmux new-session -d -s \"$session\" -n herdr",
				"exec ${(q)herdr_bin} --session default",
				"for window in herdr shell",
				"@tm-root",
				"Usage: tw [directory] [left_cmd] [top_cmd] [bottom_cmd] [right_cmd]",
				".left // \"\"",
				"prd() (",
				"command gh pr diff \"$@\" --color=never | command delta --navigate",
			},
			omit: []string{
				"keychain --eval",
				"node{{",
				"list-npm-globals",
				"alias vim=",
				"vibe() (",
				"EDITOR=$(which nvim)",
				"/aqua.yaml",
				"/aqua-checksums.json",
				"local -a roles=(work test shell)",
				"tw init [directory]",
				"$HOME/.config/pde/tw.yml",
				"local -a roles=(herdr wallace shell)",
				"@tw-root",
			},
		},
		"full": {
			profile: "full",
			want: []string{
				"vibe() (",
				"Usage: tm [hub|pocket|dash] [directory]",
				"scope=\"tm-${slug}-${hash[1,8]}\"",
				"herdr_bin=\"$(command -v herdr)\"",
				"tmux new-session -d -s \"$session\" -n herdr",
				"exec ${(q)herdr_bin} --session default",
				"for window in herdr shell",
				"@tm-root",
				"Usage: tw [directory] [left_cmd] [top_cmd] [bottom_cmd] [right_cmd]",
				".left // \"\"",
				"prd() (",
				"command gh pr diff \"$@\" --color=never | command delta --navigate",
			},
			omit: []string{
				"aqua-terminal.yaml",
				"aqua-terminal-checksums.json",
				"local -a roles=(work test shell)",
				"tw init [directory]",
				"$HOME/.config/pde/tw.yml",
				"local -a roles=(herdr wallace shell)",
				"@tw-root",
			},
		},
	}
	assertProfileTemplates(t, "dot_zshrc.tmpl", tests)
}

const (
	fakeGHScript = `#!/bin/sh
printf '%s\n' "$@" >"$HOME/gh-arguments"
printf 'diff body\n'
`
	fakeDeltaScript = `#!/bin/sh
printf '%s\n' "$@" >"$HOME/delta-arguments"
cat >"$HOME/delta-input"
`
	fakeVibeScript = `#!/bin/sh
printf '%s\n' "${GOOG_BASE_URL-}" "${GOOG_API_KEY-}" "${GOOG_MODEL-}" >"$HOME/vibe-environment"
printf '%s\n' "$@" >"$HOME/vibe-arguments"
`
)

func TestPRDForwardsDiffArguments(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	writeExecutable(t, filepath.Join(home, ".local", "bin", "gh"), fakeGHScript)
	writeExecutable(t, filepath.Join(home, ".local", "bin", "delta"), fakeDeltaScript)

	command := openCodeZshCommand(home, `prd 123 --exclude 'generated/*'`)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("prd failed: %v\n%s", err, output)
	}

	for path, want := range map[string]string{
		"gh-arguments":    "pr\ndiff\n123\n--exclude\ngenerated/*\n--color=never\n",
		"delta-arguments": "--navigate\n",
		"delta-input":     "diff body\n",
	} {
		data, err := os.ReadFile(filepath.Join(home, path))
		if err != nil {
			t.Fatal(err)
		}
		if got := string(data); got != want {
			t.Errorf("%s = %q, want %q", path, got, want)
		}
	}
}

func TestVibeScopesGoogEnvironment(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	writeExecutable(t, filepath.Join(home, ".local", "bin", "vibe"), fakeVibeScript)
	writeSecretFileAt(t, filepath.Join(home, ".config", "vibe", "goog.env"), `export GOOG_BASE_URL=https://goog.example.test
export GOOG_API_KEY=demo-key
export GOOG_MODEL=qwen3.8
`)

	command := openCodeZshCommand(home, `vibe run --key demo --model goog/qwen3.8
print -r -- "${GOOG_BASE_URL-}:${GOOG_API_KEY-}:${GOOG_MODEL-}" >"$HOME/vibe-caller"`)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("vibe failed: %v\n%s", err, output)
	}

	for path, want := range map[string]string{
		"vibe-environment": "https://goog.example.test\ndemo-key\nqwen3.8\n",
		"vibe-arguments":   "run\n--key\ndemo\n--model\ngoog/qwen3.8\n",
		"vibe-caller":      "::\n",
	} {
		data, err := os.ReadFile(filepath.Join(home, path))
		if err != nil {
			t.Fatal(err)
		}
		if got := string(data); got != want {
			t.Errorf("%s = %q, want %q", path, got, want)
		}
	}
}

func TestPRDPropagatesGitHubFailure(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	writeExecutable(t, filepath.Join(home, ".local", "bin", "gh"), "#!/bin/sh\nexit 9\n")
	writeExecutable(t, filepath.Join(home, ".local", "bin", "delta"), "#!/bin/sh\ncat >/dev/null\n")

	command := openCodeZshCommand(home, `prd 123`)
	if output, err := command.CombinedOutput(); err == nil {
		t.Fatalf("prd succeeded: %s", output)
	}
}

func openCodeZshCommand(home, script string) *exec.Cmd {
	command := exec.Command("zsh", "-i", "-c", script)
	for _, value := range os.Environ() {
		if !strings.HasPrefix(value, "HOME=") &&
			!strings.HasPrefix(value, "ZDOTDIR=") {
			command.Env = append(command.Env, value)
		}
	}
	command.Env = append(command.Env, "HOME="+home, "ZDOTDIR="+home)
	return command
}

func writeOpenCodeZshRuntime(t *testing.T, home string) {
	t.Helper()
	writeApplyFile(t, filepath.Join(home, ".zshrc"), renderProfileTemplate(t, "dot_zshrc.tmpl", "full"))
	for _, plugin := range []string{"colored-man-pages", "command-not-found", "git", "git-extras", "node"} {
		writeApplyFile(t, zshPluginPath(home, "ohmyzsh/plugins/"+plugin, plugin+".plugin.zsh"), "")
	}
	for _, path := range []string{
		zshPluginPath(home, "powerlevel10k", "powerlevel10k.zsh-theme"),
		zshPluginPath(home, "zsh-z", "zsh-z.plugin.zsh"),
		zshPluginPath(home, "zsh-autosuggestions", "zsh-autosuggestions.zsh"),
		zshPluginPath(home, "zsh-history-substring-search", "zsh-history-substring-search.zsh"),
		zshPluginPath(home, "zsh-syntax-highlighting", "zsh-syntax-highlighting.zsh"),
	} {
		writeApplyFile(t, path, "")
	}
}

func zshPluginPath(home, plugin, file string) string {
	return filepath.Join(home, ".local", "share", "zsh", "plugins", filepath.FromSlash(plugin), file)
}

func writeSecretFileAt(t *testing.T, path, contents string) {
	t.Helper()
	if err := os.MkdirAll(filepath.Dir(path), 0o700); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(path, []byte(contents), 0o600); err != nil {
		t.Fatal(err)
	}
}

func TestTmuxTemplateProfiles(t *testing.T) {
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"terminal": {profile: "terminal", want: []string{"set -s set-clipboard on", "allow-passthrough on", "@resurrect-processes 'ssh'", "set -g base-index 1"}, omit: []string{"@resurrect-strategy-vim", "@resurrect-strategy-nvim", "@resurrect-processes 'ssh vim nvim'"}},
		"full":     {profile: "full", want: []string{"@resurrect-strategy-vim", "@resurrect-strategy-nvim", "@resurrect-processes 'ssh vim nvim'", "set -s set-clipboard on", "set -g base-index 1"}, omit: []string{"@resurrect-processes 'ssh'"}},
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

func TestTemplatesRejectInvalidColors(t *testing.T) {
	if _, err := renderConfiguredTemplate(t, ".chezmoiignore.tmpl", "full", "nord"); err == nil {
		t.Fatal("render succeeded")
	}
}

func TestColorDataHasCompletePalettes(t *testing.T) {
	values := loadTemplateData(t)
	profiles, ok := values["colorProfiles"].(map[string]any)
	if !ok {
		t.Fatal("colorProfiles is not an object")
	}
	wantProfiles := colorprofile.All()
	if len(profiles) != len(wantProfiles) {
		t.Fatalf("colorProfiles count = %d, want %d", len(profiles), len(wantProfiles))
	}
	wantOpenCode := map[colorprofile.Profile]string{
		colorprofile.TokyoNight:     "tokyonight",
		colorprofile.EverforestDark: "everforest",
		colorprofile.GruvboxDark:    "gruvbox",
	}
	hexColor := regexp.MustCompile(`^#[0-9a-fA-F]{6}$`)
	for _, name := range wantProfiles {
		profileData, ok := profiles[string(name)].(map[string]any)
		if !ok {
			t.Errorf("colorProfiles[%q] is missing", name)
			continue
		}
		terminal, ok := profileData["terminal"].(map[string]any)
		if !ok {
			t.Errorf("colorProfiles[%q].terminal is missing", name)
			continue
		}
		assertColorFields(t, hexColor, terminal, "background", "foreground", "cursor", "selectionBackground", "selectionForeground")
		for _, set := range []string{"normal", "bright"} {
			palette, ok := terminal[set].(map[string]any)
			if !ok {
				t.Errorf("colorProfiles[%q].terminal.%s is missing", name, set)
				continue
			}
			assertColorFields(t, hexColor, palette, "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white")
			if len(palette) != 8 {
				t.Errorf("colorProfiles[%q].terminal.%s has %d colors, want 8", name, set, len(palette))
			}
		}
		ui, ok := profileData["ui"].(map[string]any)
		if !ok {
			t.Errorf("colorProfiles[%q].ui is missing", name)
		} else {
			assertColorFields(t, hexColor, ui, "surface", "muted", "subtle", "accent", "secondary", "success", "error", "warning")
		}
		for _, field := range []string{"label", "bat"} {
			if value, ok := profileData[field].(string); !ok || value == "" {
				t.Errorf("colorProfiles[%q].%s is missing", name, field)
			}
		}
		if got, ok := profileData["opencode"].(string); !ok || got != wantOpenCode[name] {
			t.Errorf("colorProfiles[%q].opencode = %v, want %q", name, profileData["opencode"], wantOpenCode[name])
		}
		nvim, ok := profileData["nvim"].(map[string]any)
		colorscheme, colorschemeOK := nvim["colorscheme"].(string)
		lualine, lualineOK := nvim["lualine"].(string)
		if !ok || !colorschemeOK || colorscheme == "" || !lualineOK || lualine == "" {
			t.Errorf("colorProfiles[%q].nvim is incomplete", name)
		}
	}
}

func TestColorTemplatesRenderEveryProfile(t *testing.T) {
	templates := []string{
		"dot_config/alacritty/alacritty.toml.tmpl",
		"dot_config/wezterm/wezterm.lua.tmpl",
		"dot_tmux.conf.tmpl",
		"dot_p10k.zsh.tmpl",
		"dot_zshrc.tmpl",
		"dot_config/nvim/lua/plugins/colorscheme.lua.tmpl",
		"dot_config/nvim/lua/plugins/ui.lua.tmpl",
	}
	profiles := colorprofile.All()
	for _, name := range templates {
		t.Run(name, func(t *testing.T) {
			outputs := make(map[[32]byte]string)
			for _, profile := range profiles {
				text, err := renderConfiguredTemplate(t, name, "full", string(profile))
				if err != nil {
					t.Fatalf("render %s: %v", profile, err)
				}
				if strings.Contains(text, "{{") {
					t.Fatalf("render %s left a template action", profile)
				}
				outputs[sha256.Sum256([]byte(text))] = string(profile)
			}
			if len(outputs) != len(profiles) {
				t.Fatalf("rendered %d distinct outputs, want %d", len(outputs), len(profiles))
			}
		})
	}
}

func TestPowerlevelUsesRGBColors(t *testing.T) {
	for _, profile := range colorprofile.All() {
		path := filepath.Join(t.TempDir(), ".p10k.zsh")
		text, err := renderConfiguredTemplate(t, "dot_p10k.zsh.tmpl", "full", string(profile))
		if err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(text), 0o600); err != nil {
			t.Fatal(err)
		}
		command := exec.Command("zsh", "-c", `
source "$1"
for name in ${(k)parameters}; do
  [[ $name == POWERLEVEL9K_*_(FOREGROUND|BACKGROUND) ]] || continue
  value=${(P)name}
  [[ -z $value || $value == \#[0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F][0-9a-fA-F] ]] || exit 1
done
`, "zsh", path)
		if output, err := command.CombinedOutput(); err != nil {
			t.Fatalf("%s leaves a non-RGB prompt color: %v\n%s", profile, err, output)
		}
	}
}

func TestRenderedConfigsAvoidPaletteChanges(t *testing.T) {
	for _, name := range []string{"dot_tmux.conf.tmpl", "dot_p10k.zsh.tmpl", "dot_zshrc.tmpl"} {
		text, err := renderConfiguredTemplate(t, name, "full", "everforest-dark")
		if err != nil {
			t.Fatal(err)
		}
		for _, escape := range []string{"]4;", "]10;", "]11;", "]12;"} {
			if strings.Contains(text, escape) {
				t.Errorf("%s emits terminal palette selector %q", name, escape)
			}
		}
	}
}

func TestWezTermRendersInterfaceColors(t *testing.T) {
	text, err := renderConfiguredTemplate(t, "dot_config/wezterm/wezterm.lua.tmpl", "full", "everforest-dark")
	if err != nil {
		t.Fatal(err)
	}
	for _, field := range []string{"scrollbar_thumb", "split"} {
		if !strings.Contains(text, field+" = '#") {
			t.Errorf("rendered WezTerm config omits %s", field)
		}
	}
}

func assertColorFields(t *testing.T, pattern *regexp.Regexp, values map[string]any, fields ...string) {
	t.Helper()
	for _, field := range fields {
		value, ok := values[field].(string)
		if !ok || !pattern.MatchString(value) {
			t.Errorf("%s = %v, want #RRGGBB", field, values[field])
		}
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

func assertProfileTemplateLines(t *testing.T, templateName string, tests map[string]struct {
	profile string
	want    []string
	omit    []string
}) {
	t.Helper()
	for name, test := range tests {
		t.Run(name, func(t *testing.T) {
			text := renderProfileTemplate(t, templateName, test.profile)
			for _, value := range test.want {
				if !containsLine(text, value) {
					t.Errorf("%s omits line %q", templateName, value)
				}
			}
			for _, value := range test.omit {
				if containsLine(text, value) {
					t.Errorf("%s contains line %q", templateName, value)
				}
			}
		})
	}
}

func containsLine(text, want string) bool {
	for _, line := range strings.Split(text, "\n") {
		if line == want {
			return true
		}
	}
	return false
}

func TestRenderedExternalsPinRemoteChecksums(t *testing.T) {
	text := renderProfileTemplate(t, ".chezmoiexternal.toml.tmpl", "full")
	if err := checkRemoteChecksums(text); err != nil {
		t.Fatal(err)
	}
	localCount := 0
	for _, entry := range parseExternals(text) {
		if strings.HasPrefix(entry.url, "file://") {
			localCount++
			if entry.hasChecksum {
				t.Fatal("local external has a checksum")
			}
		}
	}
	if localCount == 0 {
		t.Fatal("no local externals rendered")
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
	return renderConfiguredTemplate(t, name, profile, "tokyo-night")
}

func renderConfiguredTemplate(t *testing.T, name, profile, colorProfile string) (string, error) {
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
			if key == "PDE_COLOR_PROFILE" {
				return colorProfile
			}
			return ""
		},
		"fail": func(message string) (string, error) { return "", &templateError{message} },
	}
	tmpl, err := template.New(name).Option("missingkey=error").Funcs(functions).Parse(string(data))
	if err != nil {
		return "", err
	}
	var output bytes.Buffer
	if err := tmpl.Execute(&output, loadTemplateData(t)); err != nil {
		return "", err
	}
	return output.String(), nil
}

func loadTemplateData(t *testing.T) map[string]any {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(repoRoot(t), "chezmoi", ".chezmoidata.json"))
	if err != nil {
		t.Fatal(err)
	}
	var values map[string]any
	if err := json.Unmarshal(data, &values); err != nil {
		t.Fatal(err)
	}
	return values
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
