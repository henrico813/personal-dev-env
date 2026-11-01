package manifest

import (
	"reflect"
	"slices"
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
	items := ItemsFor("terminal")
	got := make([]inventoryItem, 0, len(items))
	for _, item := range items {
		got = append(got, inventoryItem{name: item.Name, owner: item.Owner})
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("terminal items = %v, want %v", got, want)
	}
}

// Client adds its items to terminal's, and full adds the rest. A wrongly
// tagged item would install on, or vanish from, every machine of a profile.
func TestEachProfileAddsRecordedItems(t *testing.T) {
	tests := []struct {
		profile, smaller profile.Profile
		adds             []string
	}{
		{
			profile: "client",
			smaller: "terminal",
			adds: []string{
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
			},
		},
		{
			profile: "full",
			smaller: "client",
			adds: []string{
				"gopls",
				"lua-language-server",
				"planner",
				"pde-gh-write",
				"opencode-inline-shim",
				"vibe",
				"blink.cmp",
				"pde-comment-review",
				"pde-comment-review-next",
				"pde-comment-checker",
				"go",
				"rust",
				"keychain",
				"moshi-hook",
			},
		},
	}
	for _, test := range tests {
		t.Run(string(test.profile), func(t *testing.T) {
			smaller := map[string]bool{}
			for _, item := range ItemsFor(test.smaller) {
				smaller[item.Name] = true
			}

			var adds []string
			for _, item := range ItemsFor(test.profile) {
				if !smaller[item.Name] {
					adds = append(adds, item.Name)
				}
			}

			if !reflect.DeepEqual(adds, test.adds) {
				t.Fatalf("%s adds %q, want %q", test.profile, adds, test.adds)
			}
		})
	}
}

// pde-installer list shows the manifest's Ubuntu items, and apt installs the
// profile's packages from profiles.yaml. Each listed package must install on
// exactly the profiles whose apt groups contain it.
func TestUbuntuItemsMatchProfilePackages(t *testing.T) {
	for _, selected := range []profile.Profile{"terminal", "client", "full"} {
		t.Run(string(selected), func(t *testing.T) {
			listed := map[string]bool{}
			for _, item := range ByOwnerFor(selected, Ubuntu) {
				listed[item.Name] = true
			}

			for _, item := range Items() {
				if item.Owner != Ubuntu {
					continue
				}
				installed := slices.Contains(selected.Packages(), item.Name)
				if installed != listed[item.Name] {
					t.Errorf("%s: apt installs it = %t, list shows it = %t", item.Name, installed, listed[item.Name])
				}
			}
		})
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
