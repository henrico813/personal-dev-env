package manifest

import (
	"reflect"
	"testing"

	"pde-installer/internal/profile"
)

// Each managed item must have exactly one pinned owner.
func TestManifestAssignsOneOwner(t *testing.T) {
	t.Parallel()
	if err := Validate(); err != nil {
		t.Fatalf("Validate() error = %v", err)
	}
}

// Runtime tools must appear in the installer ownership list.
func TestManifestIncludesRuntimeTools(t *testing.T) {
	t.Parallel()
	for _, name := range []string{"gopls", "lua-language-server"} {
		item, ok := Find(name, Aqua)
		if !ok || item.Version == "" {
			t.Errorf("Find(%q, Aqua) = %#v, %t", name, item, ok)
		}
	}
	for _, name := range []string{"neovim", "go", "rust", "node", "keychain"} {
		item, ok := Find(name, Direct)
		if !ok || item.Version == "" {
			t.Errorf("Find(%q, Direct) = %#v, %t", name, item, ok)
		}
	}
}

// Terminal membership is explicit so new components do not silently expand it.
func TestTerminalInventory(t *testing.T) {
	wantByBackend := []struct {
		owner Backend
		names []string
	}{
		{owner: Ubuntu, names: []string{"zsh", "git", "xclip", "unzip"}},
		{owner: Direct, names: []string{"tmux"}},
		{
			owner: Aqua,
			names: []string{
				"aqua",
				"fd",
				"fzf",
				"ripgrep",
				"bat",
				"jq",
				"gh",
				"delta",
				"wt",
				"chezmoi",
				"eza",
				"zoxide",
				"bottom",
				"yq",
				"yazi",
				"ya",
			},
		},
		{owner: Chezmoi, names: []string{"repository-config", "antidote", "tpm", "ohmyzsh", "powerlevel10k", "zsh-z", "zsh-autosuggestions", "zsh-completions", "zsh-syntax-highlighting", "zsh-history-substring-search", "tmux-sensible", "tmux-resurrect"}},
	}
	type inventoryItem struct {
		name  string
		owner Backend
	}
	var want []inventoryItem
	for _, group := range wantByBackend {
		for _, name := range group.names {
			want = append(want, inventoryItem{name: name, owner: group.owner})
		}
	}
	items := ItemsFor(profile.Terminal)
	got := make([]inventoryItem, 0, len(items))
	for _, item := range items {
		got = append(got, inventoryItem{name: item.Name, owner: item.Owner})
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("terminal items = %v, want %v", got, want)
	}
}

// Client is terminal plus what a laptop needs to edit files, run OpenCode
// and show other machines' herdr sessions. Marking Go, Rust or a compiled
// local build as client would make that weak laptop compile.
func TestClientAddsOnlyClientItems(t *testing.T) {
	terminal := map[string]bool{}
	for _, item := range ItemsFor(profile.Terminal) {
		terminal[item.Name] = true
	}

	var added []string
	for _, item := range ItemsFor(profile.Client) {
		if !terminal[item.Name] {
			added = append(added, item.Name)
		}
	}

	want := []string{
		"fontconfig",
		"@opencode/cli",
		"@openai/codex",
		"@earendil-works/pi-coding-agent",
		"obsidian-headless",
		"@anthropic-ai/claude-code",
		"skill-lessons",
		"FiraCode",
		"JetBrainsMono",
		"neovim",
		"node",
		"herdr",
		"nobs",
		"obsidian.nvim",
		"neovim-plugins",
		"ai-config",
	}
	if !reflect.DeepEqual(added, want) {
		t.Fatalf(
			"client adds %v, want %v",
			added,
			want,
		)
	}
}

func TestInvalidProfileUsesFullInventory(t *testing.T) {
	if got, want := ItemsFor(profile.Profile("desktop")), Items(); !reflect.DeepEqual(got, want) {
		t.Fatalf("invalid profile items = %v, want full inventory", got)
	}
}

func TestManifestIncludesSystemTools(t *testing.T) {
	t.Parallel()
	for _, name := range []string{"zsh", "git", "xclip", "unzip", "fontconfig"} {
		if _, ok := Find(name, Ubuntu); !ok {
			t.Errorf("Find(%q, Ubuntu) missing", name)
		}
	}
	item, ok := Find("tmux", Direct)
	if !ok || item.Version != "3.7b" {
		t.Errorf("Find(tmux, Direct) = %#v, %t", item, ok)
	}
}
