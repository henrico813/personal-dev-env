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
	"time"

	"pde-installer/internal/colorprofile"
	"pde-installer/internal/profile"
	"pde-installer/internal/testfile"
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
		".local/bin/pde-prune-targets",
		".config/systemd/user/pde-prune-targets.service",
		".config/systemd/user/pde-prune-targets.timer",
		"enable_prune_timer.sh",
		".config/systemd/user/hub-watch.service",
		"enable_hub_watch.sh",
		".config/systemd/user/pde-sweep.service",
		".config/systemd/user/pde-sweep.timer",
		"enable_sweep_timer.sh",
		".config/systemd/user/pde-update.service",
		".config/systemd/user/pde-update.timer",
		"enable_update_timer.sh",
		".config/systemd/user/pde-skill-evals.service",
		".config/systemd/user/pde-skill-evals.timer",
		"enable_skill_evals_timer.sh",
		".config/systemd/user/pde-ssh-agent.service",
		".config/environment.d",
		".config/environment.d/**",
		"enable_ssh_agent.sh",
	}
	terminalIgnores := []string{
		".config/alacritty",
		".config/alacritty/**",
		".config/wezterm",
		".config/wezterm/**",
		".config/nvim",
		".config/nvim/**",
		".config/opencode",
		".config/opencode/**",
		".local/bin/opencode",
		".local/bin/pde-quota",
		".config/herdr",
		".config/herdr/**",
		".agents",
		".agents/**",
	}
	fullIgnores := []string{
		"README.md",
		".config/nvim/pack/plugins/start/opencode-inline.nvim",
		".config/nvim/pack/plugins/start/opencode-inline.nvim/**",
		".config/aquaproj-aqua/aqua-terminal.yaml",
		".config/aquaproj-aqua/aqua-terminal-checksums.json",
		".config/nvim/pack/plugins/start/blink.cmp",
		".config/nvim/pack/plugins/start/blink.cmp/**",
	}
	sharedDesktopIgnores := []string{
		".config/pde-desktop",
		".config/pde-desktop/**",
		".config/rofi",
		".config/rofi/**",
		".config/dunst",
		".config/dunst/**",
		".config/gtk-3.0/settings.ini",
		".config/autostart/xfce4-notifyd.desktop",
	}
	i3Ignores := []string{
		".config/i3",
		".config/i3/**",
		".config/polybar",
		".config/polybar/**",
		".config/cava",
		".config/cava/**",
		".config/picom",
		".config/picom/**",
		".xinputrc",
	}
	swayIgnores := []string{
		".config/sway",
		".config/sway/**",
		".config/waybar",
		".config/waybar/**",
	}
	xWatchIgnores := []string{
		".config/autostart/pde-x-watch.desktop",
	}
	join := func(lists ...[]string) []string {
		var joined []string
		for _, list := range lists {
			joined = append(joined, list...)
		}
		return joined
	}
	tests := []struct {
		profile, desktop string
		want             []string
	}{
		{
			profile: "terminal",
			want:    join(clientIgnores, terminalIgnores, sharedDesktopIgnores, i3Ignores, swayIgnores, xWatchIgnores),
		},
		{
			profile: "client",
			want:    join(clientIgnores, sharedDesktopIgnores, i3Ignores, swayIgnores, xWatchIgnores),
		},
		{
			profile: "full",
			want:    join(fullIgnores, sharedDesktopIgnores, i3Ignores, swayIgnores, xWatchIgnores),
		},
		{
			profile: "client",
			desktop: "sway",
			want:    join(clientIgnores, i3Ignores, xWatchIgnores),
		},
		{
			profile: "full",
			desktop: "i3",
			want:    join(fullIgnores, swayIgnores),
		},
	}
	for _, test := range tests {
		t.Run(test.profile+" "+test.desktop, func(t *testing.T) {
			got := renderPathList(t, ".chezmoiignore.tmpl", test.profile, test.desktop)

			if !slices.Equal(got, test.want) {
				t.Fatalf("ignore list =\n%s\nwant\n%s", strings.Join(got, "\n"), strings.Join(test.want, "\n"))
			}
		})
	}
}

// Desktop files must reach only machines with their desktop. A source file
// under these folders that the ignore list misses installs on every
// terminal, client and full machine, or on the other desktop.
func TestIgnoreListCoversEveryDesktopFile(t *testing.T) {
	// Each folder or file maps to the desktop that owns it, or to "" when both
	// desktops share it.
	owners := map[string]string{
		".config/pde-desktop":                     "",
		".config/rofi":                            "",
		".config/dunst":                           "",
		".config/gtk-3.0/settings.ini":            "",
		".config/autostart/xfce4-notifyd.desktop": "",
		".config/i3":                              "i3",
		".config/polybar":                         "i3",
		".config/cava":                            "i3",
		".config/picom":                           "i3",
		".xinputrc":                               "i3",
	}
	files := map[string]string{}
	found := map[string]bool{}
	for _, target := range listManagedSourceTargets(t) {
		for root, owner := range owners {
			if doPathsOverlap(target, root) {
				files[target] = owner
				found[root] = true
			}
		}
	}
	for root := range owners {
		if !found[root] {
			t.Errorf("no source file installs %s", root)
		}
	}

	for _, selected := range []string{"terminal", "client", "full"} {
		for _, desktop := range []string{"", "i3", "sway"} {
			ignores := renderPathList(t, ".chezmoiignore.tmpl", selected, desktop)
			for target, owner := range files {
				want := desktop == "" || (owner != "" && owner != desktop)
				if got := isIgnored(target, ignores); got != want {
					t.Errorf("%s %q: %s ignored = %t, want %t", selected, desktop, target, got, want)
				}
			}
		}
	}
}

// Matches the two pattern forms .chezmoiignore.tmpl writes, a target path or
// a folder followed by /**.
func isIgnored(target string, patterns []string) bool {
	for _, pattern := range patterns {
		if folder, found := strings.CutSuffix(pattern, "/**"); found && strings.HasPrefix(target, folder+"/") {
			return true
		}
		if pattern == target {
			return true
		}
	}
	return false
}

// The owner sets update_interval as a Go duration, and systemd reads the
// timer. A wrong unit or key would run updates far more or less often.
func TestUpdateTimerUsesConfiguredInterval(t *testing.T) {
	data := encodeTemplateData(
		"full",
		"",
		90*time.Minute,
	)

	text, err := renderTemplateWithData(
		t,
		"dot_config/systemd/user/pde-update.timer.tmpl",
		data,
		"tokyo-night",
		findRepoRoot(t),
	)

	if err != nil {
		t.Fatal(err)
	}
	if !containsLine(text, "OnUnitActiveSec=5400s") {
		t.Fatalf(
			"timer has no OnUnitActiveSec=5400s line:\n%s",
			text,
		)
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
		".local/bin/pde-comment-checker",
		".local/bin/pde-comment-review-next",
		".local/bin/pde-comment-review",
		".local/share/pde/herdr-plugins/pde-comment-review",
		".agents/skills/roadmap-run",
		".agents/skills/promote-memory",
		".agents/skills/plain-direct-english/reference",
		".agents/skills/herdr-hub/references/waiting-on-others.md",
	}
	developmentTools := []string{
		".local/bin/gh",
		".local/bin/pde-gh",
		".local/bin/pde-gh-guard",
		".local/bin/pde-gh-write",
		".local/bin/pde-pr-approve",
		".local/bin/prs",
		".local/bin/pde-x-watch",
		".local/bin/card",
	}
	retiredAgentFiles := []string{
		".config/opencode/plugins/pde-approval-session.js",
		".config/opencode/commands/document_codebase.md",
		".config/opencode/opencode-mem.jsonc",
	}
	fullOnlyFiles := []string{
		".agents/skills/auto-run",
		".agents/skills/card-writing",
		".agents/skills/cleanup-plan",
		".agents/skills/code-documentation",
		".agents/skills/comment-review",
		".agents/skills/conclude-session",
		".agents/skills/create-plan",
		".agents/skills/design-doc",
		".agents/skills/git-messages",
		".agents/skills/go-development",
		".agents/skills/herdr-handoff",
		".agents/skills/herdr-hub",
		".agents/skills/implement-plan",
		".agents/skills/pull-request",
		".agents/skills/python-development",
		".agents/skills/research-codebase",
		".agents/skills/review-plan",
		".agents/skills/rust-development",
		".agents/skills/screen-check",
		".agents/skills/session-review",
		".agents/skills/skill-development",
		".agents/skills/skill-lessons",
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
		".agents/skills/code-documentation/scripts/check_comments.py",
		".agents/skills/code-documentation/scripts/check_python_rules.py",
		".agents/skills/comment-review/references/proposals-file.md",
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
		{profile: "terminal", want: join(everyProfile, developmentTools)},
		{profile: "client", want: join(everyProfile, developmentTools, retiredAgentFiles, fullOnlyFiles)},
		{profile: "full", want: join(everyProfile, retiredAgentFiles, retiredFullFiles)},
	}
	for _, test := range tests {
		t.Run(test.profile, func(t *testing.T) {
			got := renderPathList(t, ".chezmoiremove", test.profile, "")
			slices.Sort(got)

			if !slices.Equal(got, test.want) {
				t.Fatalf("remove list =\n%s\nwant\n%s", strings.Join(got, "\n"), strings.Join(test.want, "\n"))
			}
		})
	}
}

// Returns the rendered lines that name paths, without blanks and comments.
func renderPathList(t *testing.T, name, profileName, desktop string) []string {
	t.Helper()
	data := encodeTemplateData(
		profile.Profile(profileName),
		profile.Desktop(desktop),
		time.Hour,
	)
	text, err := renderTemplateWithData(t, name, data, "tokyo-night", findRepoRoot(t))
	if err != nil {
		t.Fatal(err)
	}
	var paths []string
	for _, line := range strings.Split(text, "\n") {
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
		"daily-shutdown",
		"delegate",
		"model-routing",
		"obsidian-daily-note",
		"obsidian-zettel",
		"plain-direct-english",
		"skill-fix",
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
		mapping := fmt.Sprintf("[%q]\ntype = \"file\"\nurl = 'file://%s/ai/AGENTS.md'", target, findRepoRoot(t))
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
	path := filepath.Join(findRepoRoot(t), "chezmoi", "dot_config", "pde", "gitconfig")
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
				`herdr_command="exec env SSH_TTY=\"\$(tty)\" ${(q)herdr_bin} --session default"`,
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
				"notes() {",
				"/aqua.yaml",
				"/aqua-checksums.json",
				"local -a roles=(work test shell)",
				"tw init [directory]",
				"$HOME/.config/pde/tw.yml",
				"local -a roles=(herdr ",
				"@tw-root",
			},
		},
		"client": {
			profile: "client",
			want: []string{
				"aqua-terminal.yaml",
				"alias vim='nvim'",
				"EDITOR=$(which nvim)",
				"notes() {",
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
				`herdr_command="exec env SSH_TTY=\"\$(tty)\" ${(q)herdr_bin} --session default"`,
				"for window in herdr shell",
				"@tm-root",
				"Usage: tw [directory] [left_cmd] [top_cmd] [bottom_cmd] [right_cmd]",
				".left // \"\"",
				"prd() (",
				"command gh pr diff \"$@\" --color=never | command delta --navigate",
				"notes() {",
			},
			omit: []string{
				"vibe() (",
				"aqua-terminal.yaml",
				"aqua-terminal-checksums.json",
				"local -a roles=(work test shell)",
				"tw init [directory]",
				"$HOME/.config/pde/tw.yml",
				"local -a roles=(herdr ",
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
	testfile.WriteExecutable(t, filepath.Join(home, ".local", "bin", "gh"), fakeGHScript)
	testfile.WriteExecutable(t, filepath.Join(home, ".local", "bin", "delta"), fakeDeltaScript)

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

// Only full installs run pde-ssh-agent, so elsewhere the socket has no agent
// behind it and every ssh from an agent shell would fail.
func TestZshenvSetsSocketOnlyOnFull(t *testing.T) {
	socket := `export SSH_AUTH_SOCK="${XDG_RUNTIME_DIR:-/run/user/$UID}/pde-ssh-agent.socket"`
	tests := map[string]struct {
		profile string
		want    []string
		omit    []string
	}{
		"full": {
			profile: "full",
			want:    []string{socket},
		},
		"client": {
			profile: "client",
			omit:    []string{"pde-ssh-agent.socket"},
		},
		"terminal": {
			profile: "terminal",
			omit:    []string{"pde-ssh-agent.socket"},
		},
	}
	assertProfileTemplates(t, "dot_zshenv.tmpl", tests)
}

func TestZshenvLoadsGoogEnvironment(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	testfile.WriteExecutable(t, filepath.Join(home, ".local", "bin", "vibe"), fakeVibeScript)
	writeSecretFileAt(t, filepath.Join(home, ".config", "vibe", "goog.env"), `export GOOG_BASE_URL=https://goog.example.test
export GOOG_API_KEY=demo-key
export GOOG_MODEL=qwen3.8
`)

	command := newAgentZshCommand(home, `vibe run --key demo --model goog/qwen3.8
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

// ~/.zshenv must put ~/.local/bin on PATH, or comment review on a Herdr
// server started over ssh cannot open nvim.
func TestZshenvPutsLocalBinOnPath(t *testing.T) {
	home := t.TempDir()
	writeApplyFile(
		t,
		filepath.Join(home, ".zshenv"),
		renderProfileTemplate(t, "dot_zshenv.tmpl", "client"),
	)
	testfile.WriteExecutable(
		t,
		filepath.Join(home, ".local", "bin", "nvim"),
		"#!/bin/sh\n",
	)
	command := exec.Command(
		"zsh",
		"-c",
		"nvim --version",
	)
	command.Env = buildServerEnvironment(home)

	output, err := command.CombinedOutput()

	if err != nil {
		t.Fatalf(
			"zsh did not find nvim in ~/.local/bin: %v\n%s",
			err,
			output,
		)
	}
}

func TestPRDPropagatesGitHubFailure(t *testing.T) {
	home := t.TempDir()
	writeOpenCodeZshRuntime(t, home)
	testfile.WriteExecutable(t, filepath.Join(home, ".local", "bin", "gh"), "#!/bin/sh\nexit 9\n")
	testfile.WriteExecutable(t, filepath.Join(home, ".local", "bin", "delta"), "#!/bin/sh\ncat >/dev/null\n")

	command := openCodeZshCommand(home, `prd 123`)
	if output, err := command.CombinedOutput(); err == nil {
		t.Fatalf("prd succeeded: %s", output)
	}
}

// Each fzf line starts with c, p or r for a card, plan or research note. A
// wrong one hides a note from its list, and a wrong path opens the wrong note.
func TestNotesOpensPickFromLabeledList(t *testing.T) {
	tests := map[string]struct {
		script     string
		wantInput  string
		wantArgs   string
		wantOpened string
	}{
		"type c lists only cards": {
			script:     `n c card`,
			wantInput:  "c\tExample/A card.md\n",
			wantArgs:   "--query\ncard\n",
			wantOpened: "Example/A card.md",
		},
		"type r lists RESEARCH file names": {
			script: `n r`,
			wantInput: "r\tExample/Research.md\n" +
				"r\tExample/RESEARCH-Old findings.md\n",
			wantArgs:   "--query\n\n",
			wantOpened: "Example/Research.md",
		},
		"no type lists every note": {
			script: `notes`,
			wantInput: "c\tExample/A card.md\n" +
				"r\tExample/Research.md\n" +
				"-\tExample/Design.md\n" +
				"p\tExample/A plan.md\n" +
				"r\tExample/RESEARCH-Old findings.md\n",
			wantArgs:   "--query\n\n",
			wantOpened: "Example/A card.md",
		},
	}
	for name, tt := range tests {
		t.Run(name, func(t *testing.T) {
			home := t.TempDir()
			writeOpenCodeZshRuntime(
				t,
				home,
			)
			writeNotesVault(
				t,
				home,
			)
			writeUntypedResearchNote(
				t,
				home,
			)
			writeNotesTools(
				t,
				home,
			)

			command := openCodeZshCommand(
				home,
				tt.script,
			)
			if output, err := command.CombinedOutput(); err != nil {
				t.Fatalf(
					"%s failed: %v\n%s",
					tt.script,
					err,
					output,
				)
			}

			input := readHomeFile(
				t,
				home,
				"fzf-input",
			)
			if input != tt.wantInput {
				t.Errorf(
					"fzf input = %q, want %q",
					input,
					tt.wantInput,
				)
			}
			args := readHomeFile(
				t,
				home,
				"fzf-arguments",
			)
			if !strings.Contains(
				args,
				tt.wantArgs,
			) {
				t.Errorf(
					"fzf arguments = %q, want %q",
					args,
					tt.wantArgs,
				)
			}
			wantOpened := filepath.Join(
				home,
				"vault/500 Zettelkasten/Projects",
				tt.wantOpened,
			) + "\n"
			opened := readHomeFile(
				t,
				home,
				"nvim-arguments",
			)
			if opened != wantOpened {
				t.Errorf(
					"nvim arguments = %q, want %q",
					opened,
					wantOpened,
				)
			}
		})
	}
}

// Writes four notes oldest first, one per type, with a type line quoted in a
// body and Windows line endings.
func writeNotesVault(t *testing.T, home string) {
	t.Helper()
	projects := filepath.Join(
		home,
		"vault",
		"500 Zettelkasten",
		"Projects",
		"Example",
	)
	notes := []struct {
		name    string
		content string
	}{
		{
			name:    "A plan.md",
			content: "---\ntype: issue\n---\n\n# A plan\n",
		},
		{
			name:    "Design.md",
			content: "---\nproject: Example\n---\n\n```yaml\ntype: card\n```\n",
		},
		{
			name:    "Research.md",
			content: "---\ntype: research\n---\n\n# Research\n",
		},
		{
			name:    "A card.md",
			content: "---\r\ntype: \"card\"\r\nstage: ready\r\n---\r\n\r\n# A card\r\n",
		},
	}
	modified := time.Now().Add(-time.Hour)
	for _, note := range notes {
		path := filepath.Join(
			projects,
			note.name,
		)
		writeApplyFile(
			t,
			path,
			note.content,
		)
		if err := os.Chtimes(
			path,
			modified,
			modified,
		); err != nil {
			t.Fatal(err)
		}
		modified = modified.Add(time.Minute)
	}
}

// Writes a RESEARCH note with no type line, older than the notes of
// writeNotesVault.
func writeUntypedResearchNote(t *testing.T, home string) {
	t.Helper()
	path := filepath.Join(
		home,
		"vault/500 Zettelkasten/Projects/Example/RESEARCH-Old findings.md",
	)
	writeApplyFile(
		t,
		path,
		"---\nproject: Example\n---\n\n# Old findings\n",
	)
	modified := time.Now().Add(-2 * time.Hour)
	if err := os.Chtimes(
		path,
		modified,
		modified,
	); err != nil {
		t.Fatal(err)
	}
}

// Writes fake nobs, fzf and nvim that save what they receive, with fzf picking
// the first line.
func writeNotesTools(t *testing.T, home string) {
	t.Helper()
	bin := filepath.Join(
		home,
		".local",
		"bin",
	)
	testfile.WriteExecutable(
		t,
		filepath.Join(
			bin,
			"nobs",
		),
		"#!/bin/sh\nprintf '%s\\n' \"$HOME/vault\"\n",
	)
	testfile.WriteExecutable(
		t,
		filepath.Join(
			bin,
			"fzf",
		),
		`#!/bin/sh
cat >"$HOME/fzf-input"
printf '%s\n' "$@" >"$HOME/fzf-arguments"
head -n 1 "$HOME/fzf-input"
`,
	)
	testfile.WriteExecutable(
		t,
		filepath.Join(
			bin,
			"nvim",
		),
		"#!/bin/sh\nprintf '%s\\n' \"$@\" >\"$HOME/nvim-arguments\"\n",
	)
}

// Returns the text of a file a fake tool wrote under home.
func readHomeFile(t *testing.T, home, name string) string {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(
		home,
		name,
	))
	if err != nil {
		t.Fatal(err)
	}
	return string(data)
}

func newAgentZshCommand(home, script string) *exec.Cmd {
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
		writeApplyFile(t, joinZshPluginPath(home, "ohmyzsh/plugins/"+plugin, plugin+".plugin.zsh"), "")
	}
	for _, path := range []string{
		joinZshPluginPath(home, "powerlevel10k", "powerlevel10k.zsh-theme"),
		joinZshPluginPath(home, "zsh-z", "zsh-z.plugin.zsh"),
		joinZshPluginPath(home, "zsh-autosuggestions", "zsh-autosuggestions.zsh"),
		joinZshPluginPath(home, "zsh-history-substring-search", "zsh-history-substring-search.zsh"),
		joinZshPluginPath(home, "zsh-syntax-highlighting", "zsh-syntax-highlighting.zsh"),
	} {
		writeApplyFile(t, path, "")
	}
}

func joinZshPluginPath(home, plugin, file string) string {
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
	wantProfiles := colorprofile.List()
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
	profiles := colorprofile.List()
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
	for _, profile := range colorprofile.List() {
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

// Extra skills may hold private text, so they must reach only the PC whose
// config.json names their checkout. Without the setting, the personal skills
// install alone.
func TestExtraSkillsInstallOnlyWithSetting(t *testing.T) {
	personal := makeTrackedCheckout(
		t,
		"ai/skills/garden/SKILL.md",
	)
	extra := makeTrackedCheckout(
		t,
		"skills/harvest/SKILL.md",
	)
	tests := []struct {
		name    string
		setting string
		want    []string
	}{
		{
			name:    "set",
			setting: extra,
			want: []string{
				".agents/skills/garden/SKILL.md",
				".agents/skills/harvest/SKILL.md",
			},
		},
		{
			name:    "unset",
			setting: "",
			want: []string{
				".agents/skills/garden/SKILL.md",
			},
		},
	}
	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			t.Setenv(
				"PDE_EXTRA_SKILLS_REPO",
				tt.setting,
			)

			text, err := renderTemplateForCheckout(
				t,
				".chezmoiexternal.toml.tmpl",
				"full",
				"tokyo-night",
				personal,
			)

			if err != nil {
				t.Fatal(err)
			}
			var skillTargets []string
			for _, target := range renderedExternalTargets(text) {
				if strings.HasPrefix(
					target,
					".agents/skills/",
				) {
					skillTargets = append(
						skillTargets,
						target,
					)
				}
			}
			slices.Sort(skillTargets)
			if !slices.Equal(
				skillTargets,
				tt.want,
			) {
				t.Fatalf(
					"skill targets = %q, want %q",
					skillTargets,
					tt.want,
				)
			}
			url := fmt.Sprintf(
				"url = 'file://%s/skills/harvest/SKILL.md'",
				extra,
			)
			if tt.setting != "" && !containsLine(
				text,
				url,
			) {
				t.Fatalf(
					"rendered externals omit %s",
					url,
				)
			}
		})
	}
}

// A terminal install has no agents, and its ignore list drops the whole
// .agents folder, so even a PC that names a extra checkout gets no extra skill.
func TestTerminalInstallsNoExtraSkill(t *testing.T) {
	personal := makeTrackedCheckout(
		t,
		"ai/skills/garden/SKILL.md",
	)
	extra := makeTrackedCheckout(
		t,
		"skills/harvest/SKILL.md",
	)
	t.Setenv(
		"PDE_EXTRA_SKILLS_REPO",
		extra,
	)

	text, err := renderTemplateForCheckout(
		t,
		".chezmoiexternal.toml.tmpl",
		"terminal",
		"tokyo-night",
		personal,
	)

	if err != nil {
		t.Fatal(err)
	}
	ignores := renderPathList(
		t,
		".chezmoiignore.tmpl",
		"terminal",
		"",
	)
	var installed []string
	for _, target := range renderedExternalTargets(text) {
		if !strings.HasPrefix(
			target,
			".agents/skills/",
		) {
			continue
		}
		if !isIgnored(
			target,
			ignores,
		) {
			installed = append(
				installed,
				target,
			)
		}
	}
	if len(installed) != 0 {
		t.Fatalf("terminal installs skills %q, want none", installed)
	}
}

// An extra skill named like a personal one would overwrite it, or be
// overwritten, on every install. The install stops instead, and names both
// folders so the owner knows which one to rename.
func TestExternalsRefuseClashingExtraSkill(t *testing.T) {
	personal := makeTrackedCheckout(
		t,
		"ai/skills/garden/SKILL.md",
	)
	extra := makeTrackedCheckout(
		t,
		"skills/garden/SKILL.md",
	)
	t.Setenv(
		"PDE_EXTRA_SKILLS_REPO",
		extra,
	)

	_, err := renderTemplateForCheckout(
		t,
		".chezmoiexternal.toml.tmpl",
		"full",
		"tokyo-night",
		personal,
	)

	if err == nil {
		t.Fatal("render error = nil, want the name clash")
	}
	for _, folder := range []string{
		extra + "/skills/garden",
		personal + "/ai/skills/garden",
	} {
		if !strings.Contains(
			err.Error(),
			folder,
		) {
			t.Errorf(
				"render error = %q, want it to name %s",
				err,
				folder,
			)
		}
	}
}

// Returns a new git checkout that tracks an empty file at each path.
func makeTrackedCheckout(t *testing.T, paths ...string) string {
	t.Helper()
	checkout := t.TempDir()
	for _, path := range paths {
		full := filepath.Join(
			checkout,
			path,
		)
		if err := os.MkdirAll(
			filepath.Dir(full),
			0o755,
		); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(
			full,
			nil,
			0o644,
		); err != nil {
			t.Fatal(err)
		}
	}
	for _, args := range [][]string{
		{"init", "-q"},
		append(
			[]string{"add"},
			paths...,
		),
	} {
		command := exec.Command(
			"git",
			append(
				[]string{
					"-C",
					checkout,
				},
				args...,
			)...,
		)
		if output, err := command.CombinedOutput(); err != nil {
			t.Fatalf(
				"git %v failed: %v\n%s",
				args,
				err,
				output,
			)
		}
	}
	return checkout
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
	return renderTemplateForCheckout(t, name, profile, colorProfile, findRepoRoot(t))
}

func renderTemplateForCheckout(t *testing.T, name, profileName, colorProfile, checkout string) (string, error) {
	t.Helper()
	data := encodeTemplateData(
		profile.Profile(profileName),
		"",
		time.Hour,
	)
	return renderTemplateWithData(
		t,
		name,
		data,
		colorProfile,
		checkout,
	)
}

// Renders a template with pdeData, the installer's --override-data JSON.
func renderTemplateWithData(t *testing.T, name, pdeData, colorProfile, checkout string) (string, error) {
	t.Helper()
	data, err := os.ReadFile(filepath.Join(findRepoRoot(t), "chezmoi", name))
	if err != nil {
		t.Fatal(err)
	}
	values := loadTemplateData(t)
	// Decoding the installer's JSON gives .pde the types chezmoi gives it.
	if err := json.Unmarshal([]byte(pdeData), &values); err != nil {
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
			// Tests set the extra skills checkout with t.Setenv, as install does.
			if key == "PDE_EXTRA_SKILLS_REPO" {
				return os.Getenv(key)
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
		"contains": func(substring, text string) bool {
			return strings.Contains(
				text,
				substring,
			)
		},
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
	data, err := os.ReadFile(filepath.Join(findRepoRoot(t), "chezmoi", ".chezmoidata.json"))
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

func findRepoRoot(t *testing.T) string {
	t.Helper()
	root, err := filepath.Abs(filepath.Join("..", "..", ".."))
	if err != nil {
		t.Fatal(err)
	}
	return root
}
