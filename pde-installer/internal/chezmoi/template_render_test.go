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
	"slices"
	"strings"
	"testing"
	"text/template"

	"pde-installer/internal/colorprofile"
	"pde-installer/internal/profile"
)

// Each profile's whole ignore list. A path missing here installs on a machine
// that should not get it.
func TestIgnoreListMatchesEachProfile(t *testing.T) {
	clientIgnores := []string{
		"README.md",
		".config/nvim/pack/plugins/start/opencode-inline.nvim",
		".config/nvim/pack/plugins/start/opencode-inline.nvim/**",
		".config/aquaproj-aqua/aqua.yaml",
		".config/aquaproj-aqua/aqua-checksums.json",
		".claude",
		".claude/**",
		".codex",
		".codex/**",
		".pi",
		".pi/**",
		".local/share/pde/herdr-plugins",
		".local/share/pde/herdr-plugins/**",
		"link_herdr_plugins.sh",
		".config/opencode/bin/planner",
	}
	tests := []struct {
		profile string
		want    []string
	}{
		{
			profile: "terminal",
			want: append(slices.Clone(clientIgnores),
				".config/alacritty",
				".config/alacritty/**",
				".config/wezterm",
				".config/wezterm/**",
				".config/nvim",
				".config/nvim/**",
				".config/opencode",
				".config/opencode/**",
				".local/bin/opencode",
				".config/herdr",
				".config/herdr/**",
				".agents",
				".agents/**",
			),
		},
		{
			profile: "client",
			want:    clientIgnores,
		},
		{
			profile: "full",
			want: []string{
				"README.md",
				".config/nvim/pack/plugins/start/opencode-inline.nvim",
				".config/nvim/pack/plugins/start/opencode-inline.nvim/**",
				".config/aquaproj-aqua/aqua-terminal.yaml",
				".config/aquaproj-aqua/aqua-terminal-checksums.json",
				".config/nvim/pack/plugins/start/blink.cmp",
				".config/nvim/pack/plugins/start/blink.cmp/**",
			},
		},
	}
	for _, test := range tests {
		t.Run(test.profile, func(t *testing.T) {
			got := renderPathList(t, ".chezmoiignore.tmpl", test.profile)

			if !slices.Equal(got, test.want) {
				t.Fatalf("ignore list =\n%s\nwant\n%s", strings.Join(got, "\n"), strings.Join(test.want, "\n"))
			}
		})
	}
}

// Each profile's remove list. chezmoi deletes these targets on every apply, so
// a tool listed for a profile that installs it, such as pde-gh-write on full,
// disappears after each install.
func TestRemoveListMatchesEachProfile(t *testing.T) {
	everyProfile := []string{
		".config/pde/paths.env",
		".config/systemd/user/opencode-web.service",
		".config/systemd/user/opencode-web-health.service",
		".config/systemd/user/opencode-web-health.timer",
		".local/bin/surveil",
		".local/state/surveil",
	}
	approvalTools := []string{
		".local/bin/pde-gh-write",
		".local/bin/pde-pr-approve",
		".local/bin/pde-comment-checker",
	}
	retiredAgentFiles := []string{
		".config/opencode/plugins/pde-approval-session.js",
		".config/opencode/commands/document_codebase.md",
	}
	fullOnlyFiles := []string{
		".agents/skills/cleanup-plan",
		".agents/skills/code-documentation",
		".agents/skills/comment-review",
		".agents/skills/create-plan",
		".agents/skills/design-doc",
		".agents/skills/git-messages",
		".agents/skills/go-development",
		".agents/skills/implement-plan",
		".agents/skills/promote-memory",
		".agents/skills/pull-request",
		".agents/skills/python-development",
		".agents/skills/research-codebase",
		".agents/skills/review-plan",
		".agents/skills/roadmap-run",
		".agents/skills/rust-development",
		".agents/skills/typescript-development",
		".config/opencode/commands/cleanup_plan.md",
		".config/opencode/commands/create_plan.md",
		".config/opencode/commands/design_doc.md",
		".config/opencode/commands/implement_plan.md",
		".config/opencode/commands/research_codebase.md",
		".config/opencode/commands/review_plan.md",
		".config/opencode/agents/plan-architecture-reviewer.md",
		".config/opencode/agents/plan-bug-reviewer.md",
		".config/opencode/agents/plan-completeness-reviewer.md",
	}
	retiredFullFiles := []string{
		"Projects/CLAUDE.md",
		".codex/skills/behavior-focused-testing",
		".codex/skills/code-documentation",
		".codex/skills/git-messages",
		".codex/skills/go-development",
		".codex/skills/obsidian-zettel",
		".codex/skills/promote-memory",
		".codex/skills/python-development",
		".codex/skills/rust-development",
		".codex/skills/document-codebase",
		".codex/skills/create-plan",
		".codex/skills/review-plan",
		".codex/skills/implement-plan",
		".codex/skills/cleanup-plan",
		".codex/skills/design-doc",
		".codex/skills/research-codebase",
		".claude/commands/create_plan.md",
		".claude/commands/review_plan.md",
		".claude/commands/implement_plan.md",
		".claude/commands/cleanup_plan.md",
		".claude/commands/design_doc.md",
		".claude/commands/research_codebase.md",
		".claude/commands/document_codebase.md",
		".claude/commands/README.md",
		".agents/skills/git-messages/scripts/check-message",
		".agents/skills/code-search",
		".agents/skills/code-documentation/scripts/comment_checker",
	}
	join := func(lists ...[]string) []string {
		var result []string
		for _, list := range lists {
			result = append(result, list...)
		}
		slices.Sort(result)
		return result
	}
	tests := []struct {
		profile string
		want    []string
	}{
		{profile: "terminal", want: join(everyProfile, approvalTools)},
		{profile: "client", want: join(everyProfile, approvalTools, retiredAgentFiles, fullOnlyFiles)},
		{profile: "full", want: join(everyProfile, retiredAgentFiles, retiredFullFiles)},
	}
	for _, test := range tests {
		t.Run(test.profile, func(t *testing.T) {
			got := renderPathList(t, ".chezmoiremove", test.profile)
			slices.Sort(got)

			if !slices.Equal(got, test.want) {
				t.Fatalf("remove list =\n%s\nwant\n%s", strings.Join(got, "\n"), strings.Join(test.want, "\n"))
			}
		})
	}
}

// Returns the rendered lines that name paths, without blanks and comments.
func renderPathList(t *testing.T, name, profile string) []string {
	t.Helper()
	var paths []string
	for _, line := range strings.Split(renderProfileTemplate(t, name, profile), "\n") {
		if line != "" && !strings.HasPrefix(line, "#") {
			paths = append(paths, line)
		}
	}
	return paths
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
		"client": {
			profile: "client",
			want: []string{
				"obsidian.nvim",
				"copilot.lua",
				".config/opencode/AGENTS.md",
				".config/nvim/pack/plugins/start/blink.cmp",
			},
			omit: []string{
				".claude/CLAUDE.md",
				".pi/agent",
				".config/opencode/agents/plan-bug-reviewer.md",
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
			omit: []string{
				".config/nvim/pack/plugins/start/blink.cmp",
			},
		},
	}
	assertProfileTemplates(t, ".chezmoiexternal.toml.tmpl", tests)
}

// A client laptop is not a development machine, and most skills call planner,
// vibe or Go tools it lacks. The allow list keeps a new skill off it until
// someone adds that skill on purpose.
func TestClientGetsOnlyAllowedSkills(t *testing.T) {
	text := renderProfileTemplate(t, ".chezmoiexternal.toml.tmpl", "client")

	var skills []string
	for _, target := range renderedExternalTargets(text) {
		if name, found := strings.CutPrefix(target, ".agents/skills/"); found {
			skill, _, _ := strings.Cut(name, "/")
			if !slices.Contains(skills, skill) {
				skills = append(skills, skill)
			}
		}
	}

	slices.Sort(skills)
	want := []string{
		"behavior-focused-testing",
		"delegate",
		"obsidian-daily-note",
		"obsidian-zettel",
		"plain-direct-english",
		"skill-lessons",
		"status-updates",
	}
	if !slices.Equal(skills, want) {
		t.Fatalf("client skills = %q, want %q", skills, want)
	}
}

// TestSharedWorkflowMappings catches rules and skills diverging between tools,
// including accidental duplicate shared copies under Codex.
func TestSharedWorkflowMappings(t *testing.T) {
	text := renderProfileTemplate(t, ".chezmoiexternal.toml.tmpl", "full")
	for _, target := range []string{".config/opencode/AGENTS.md", ".codex/AGENTS.md", ".pi/agent/AGENTS.md", ".claude/CLAUDE.md"} {
		mapping := fmt.Sprintf("[%q]\ntype = \"file\"\nurl = 'file://%s/ai/AGENTS.md'", target, repoRoot(t))
		if strings.Count(text, mapping) != 1 {
			t.Fatalf("mapping %s does not point once to ai/AGENTS.md", target)
		}
	}
	for _, block := range strings.Split(text, "\n\n") {
		if strings.HasPrefix(block, `[".codex/skills/`) && strings.Contains(block, "/ai/skills/") {
			t.Fatalf("shared skill remains mapped into Codex: %s", block)
		}
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
				"plan() {",
				"/aqua.yaml",
				"/aqua-checksums.json",
				"local -a roles=(work test shell)",
				"tw init [directory]",
				"$HOME/.config/pde/tw.yml",
				"local -a roles=(herdr wallace shell)",
				"@tw-root",
			},
		},
		"client": {
			profile: "client",
			want: []string{
				"aqua-terminal.yaml",
				"alias vim='nvim'",
				"EDITOR=$(which nvim)",
				"plan() {",
			},
			omit: []string{
				"keychain --eval",
			},
		},
		"full": {
			profile: "full",
			want: []string{
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
				"plan() {",
			},
			omit: []string{
				"vibe() (",
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

func TestZshenvLoadsGoogEnvironment(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	writeExecutable(t, filepath.Join(home, ".local", "bin", "vibe"), fakeVibeScript)
	writeSecretFileAt(t, filepath.Join(home, ".config", "vibe", "goog.env"), `export GOOG_BASE_URL=https://goog.example.test
export GOOG_API_KEY=demo-key
export GOOG_MODEL=qwen3.8
`)

	command := agentZshCommand(home, `vibe run --key demo --model goog/qwen3.8
print -r -- "${GOOG_BASE_URL-}:${GOOG_API_KEY-}:${GOOG_MODEL-}" >"$HOME/vibe-caller"`)
	if output, err := command.CombinedOutput(); err != nil {
		t.Fatalf("vibe failed: %v\n%s", err, output)
	}

	for path, want := range map[string]string{
		"vibe-environment": "https://goog.example.test\ndemo-key\nqwen3.8\n",
		"vibe-arguments":   "run\n--key\ndemo\n--model\ngoog/qwen3.8\n",
		"vibe-caller":      "https://goog.example.test:demo-key:qwen3.8\n",
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

func agentZshCommand(home, script string) *exec.Cmd {
	command := exec.Command("zsh", "-c", script)
	configureZshCommand(command, home)
	return command
}

func openCodeZshCommand(home, script string) *exec.Cmd {
	command := exec.Command("zsh", "-i", "-c", script)
	configureZshCommand(command, home)
	return command
}

func configureZshCommand(command *exec.Cmd, home string) {
	hostPath := os.Getenv("PATH")
	for _, value := range os.Environ() {
		if strings.HasPrefix(value, "HOME=") ||
			strings.HasPrefix(value, "ZDOTDIR=") ||
			strings.HasPrefix(value, "GOOG_") ||
			strings.HasPrefix(value, "PATH=") {
			continue
		}
		command.Env = append(command.Env, value)
	}
	command.Env = append(command.Env,
		"HOME="+home,
		"ZDOTDIR="+home,
		"PATH="+filepath.Join(home, ".local", "bin")+":"+hostPath,
	)
}

func writeOpenCodeZshRuntime(t *testing.T, home string) {
	t.Helper()
	writeApplyFile(t, filepath.Join(home, ".zshenv"), renderProfileTemplate(t, "dot_zshenv.tmpl", "full"))
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
		"client": {
			profile: "client",
			want:    []string{"@resurrect-strategy-nvim"},
		},
		"full": {profile: "full", want: []string{"@resurrect-strategy-vim", "@resurrect-strategy-nvim", "@resurrect-processes 'ssh vim nvim'", "set -s set-clipboard on", "set -g base-index 1"}, omit: []string{"@resurrect-processes 'ssh'"}},
	}
	assertProfileTemplates(t, "dot_tmux.conf.tmpl", tests)
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

// Each skill file once needed its own entry in the template, and a missed
// one left agents told to run a comment checker that was never installed.
// Entries now come from the files Git tracks, so a tracked file in any
// folder of a skill installs and an untracked one does not.
func TestExternalsInstallEverySkillFile(t *testing.T) {
	checkout := t.TempDir()
	for name, text := range map[string]string{
		"ai/skills/demo/SKILL.md":         "tracked\n",
		"ai/skills/demo/scripts/check.py": "tracked\n",
		"ai/skills/demo/notes.md":         "untracked\n",
	} {
		path := filepath.Join(checkout, name)
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(text), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	for _, args := range [][]string{{"init", "-q"}, {"add", "ai/skills/demo/SKILL.md", "ai/skills/demo/scripts/check.py"}} {
		command := exec.Command("git", append([]string{"-C", checkout}, args...)...)
		if output, err := command.CombinedOutput(); err != nil {
			t.Fatalf("git %v failed: %v\n%s", args, err, output)
		}
	}

	text, err := renderTemplateForCheckout(t, ".chezmoiexternal.toml.tmpl", "full", "tokyo-night", checkout)
	if err != nil {
		t.Fatal(err)
	}

	var skillTargets []string
	for _, target := range renderedExternalTargets(text) {
		if strings.HasPrefix(target, ".agents/skills/") {
			skillTargets = append(skillTargets, target)
		}
	}
	slices.Sort(skillTargets)
	want := []string{".agents/skills/demo/SKILL.md", ".agents/skills/demo/scripts/check.py"}
	if !slices.Equal(skillTargets, want) {
		t.Fatalf("skill targets = %q, want %q", skillTargets, want)
	}
	url := fmt.Sprintf("url = 'file://%s/ai/skills/demo/scripts/check.py'", checkout)
	if !containsLine(text, url) {
		t.Fatalf("rendered externals omit %s", url)
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
	return renderTemplateForCheckout(t, name, profile, colorProfile, repoRoot(t))
}

func renderTemplateForCheckout(t *testing.T, name, profileName, colorProfile, checkout string) (string, error) {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(repoRoot(t), "chezmoi", name))
	if err != nil {
		t.Fatal(err)
	}
	values := loadTemplateData(t)
	// Decoding the installer's JSON gives .pde the types chezmoi gives it.
	if err := json.Unmarshal([]byte(encodeTemplateData(profile.Profile(profileName))), &values); err != nil {
		t.Fatal(err)
	}
	functions := template.FuncMap{
		"env": func(key string) string {
			if key == "PDE_REPO_ROOT" {
				return checkout
			}
			if key == "PDE_COLOR_PROFILE" {
				return colorProfile
			}
			return ""
		},
		"fail": func(message string) (string, error) { return "", &templateError{message} },
		"output": func(name string, args ...string) (string, error) {
			output, err := exec.Command(name, args...).Output()
			return string(output), err
		},
		"splitList": func(separator, text string) []string { return strings.Split(text, separator) },
		"has":       func(needle any, list []any) bool { return slices.Contains(list, needle) },
	}
	tmpl, err := template.New(name).Option("missingkey=error").Funcs(functions).Parse(string(data))
	if err != nil {
		return "", err
	}
	var output bytes.Buffer
	if err := tmpl.Execute(&output, values); err != nil {
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
