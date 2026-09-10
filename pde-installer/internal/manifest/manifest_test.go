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

func TestTerminalInventory(t *testing.T) {
	want := []string{"zsh", "git", "xclip", "unzip", "tmux", "aqua", "fd", "fzf", "ripgrep", "bat", "jq", "chezmoi", "eza", "zoxide", "bottom", "yq", "yazi", "repository-config", "antidote", "tpm", "ohmyzsh", "powerlevel10k", "zsh-z", "zsh-autosuggestions", "zsh-completions", "zsh-syntax-highlighting", "zsh-history-substring-search", "tmux-sensible", "tmux-resurrect"}
	items := ItemsFor(profile.Terminal)
	got := make([]string, 0, len(items))
	for _, item := range items {
		got = append(got, item.Name)
	}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("terminal items = %v, want %v", got, want)
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
