package manifest

import (
	"fmt"

	"pde-installer/internal/profile"
)

// Backend identifies the system responsible for an item.
type Backend string

// Supported ownership backends.
const (
	Ubuntu  Backend = "ubuntu"
	Aqua    Backend = "aqua"
	NPM     Backend = "npm"
	Local   Backend = "local"
	Direct  Backend = "direct"
	Chezmoi Backend = "chezmoi"
)

// Item describes ownership and the requested version of a managed item.
type Item struct {
	Name, Version string
	Owner         Backend
	Terminal      bool
}

// ByOwner returns items assigned to one backend.
func ByOwner(owner Backend) []Item {
	return ByOwnerFor(profile.Full, owner)
}

// ItemsFor returns items selected by the installation profile.
func ItemsFor(selected profile.Profile) []Item {
	if selected == profile.Full {
		return Items()
	}
	var result []Item
	for _, item := range Items() {
		if item.Terminal {
			result = append(result, item)
		}
	}
	return result
}

// ByOwnerFor returns selected items assigned to one backend.
func ByOwnerFor(selected profile.Profile, owner Backend) []Item {
	var result []Item
	for _, item := range ItemsFor(selected) {
		if item.Owner == owner {
			result = append(result, item)
		}
	}
	return result
}

// Find returns one item assigned to a backend.
func Find(name string, owner Backend) (Item, bool) {
	for _, item := range Items() {
		if item.Name == name && item.Owner == owner {
			return item, true
		}
	}
	return Item{}, false
}

// Items returns every item managed by the installer.
func Items() []Item {
	return []Item{
		{Name: "zsh", Owner: Ubuntu, Terminal: true}, {Name: "git", Owner: Ubuntu, Terminal: true},
		{Name: "xclip", Owner: Ubuntu, Terminal: true}, {Name: "unzip", Owner: Ubuntu, Terminal: true},
		{Name: "fontconfig", Owner: Ubuntu},
		{Name: "tmux", Version: "3.7b", Owner: Direct, Terminal: true},
		{Name: "aqua", Version: "v2.60.1", Owner: Aqua, Terminal: true},
		{Name: "fd", Version: "v8.3.1", Owner: Aqua, Terminal: true}, {Name: "fzf", Version: "0.36.0", Owner: Aqua, Terminal: true},
		{Name: "ripgrep", Version: "14.1.1", Owner: Aqua, Terminal: true}, {Name: "bat", Version: "v0.19.0", Owner: Aqua, Terminal: true},
		{Name: "jq", Version: "jq-1.7.1", Owner: Aqua, Terminal: true}, {Name: "chezmoi", Version: "v2.72.0", Owner: Aqua, Terminal: true},
		{Name: "eza", Version: "v0.23.4", Owner: Aqua, Terminal: true}, {Name: "zoxide", Version: "v0.9.8", Owner: Aqua, Terminal: true},
		{Name: "bottom", Version: "0.11.4", Owner: Aqua, Terminal: true}, {Name: "yq", Version: "v4.53.3", Owner: Aqua, Terminal: true},
		{Name: "yazi", Version: "v25.5.31", Owner: Aqua, Terminal: true},
		{Name: "gopls", Version: "v0.23.0", Owner: Aqua},
		{Name: "lua-language-server", Version: "3.19.1", Owner: Aqua},
		{Name: "opencode-ai", Version: "1.18.27", Owner: NPM},
		{Name: "@openai/codex", Version: "0.153.2", Owner: NPM},
		{Name: "@earendil-works/pi-coding-agent", Version: "0.84.4", Owner: NPM},
		{Name: "obsidian-headless", Version: "0.0.14", Owner: NPM},
		{Name: "planner", Owner: Local}, {Name: "opencode-inline-shim", Owner: Local},
		{Name: "surveil", Owner: Local}, {Name: "vibe", Owner: Local}, {Name: "blink.cmp", Owner: Local},
		{Name: "FiraCode", Version: "v3.2.1", Owner: Direct},
		{Name: "JetBrainsMono", Version: "v3.2.1", Owner: Direct},
		{Name: "neovim", Version: "0.12.3", Owner: Direct},
		{Name: "go", Version: "1.26.4", Owner: Direct},
		{Name: "rust", Version: "1.96.0", Owner: Direct},
		{Name: "node", Version: "24.16.0", Owner: Direct},
		{Name: "keychain", Version: "2.9.8", Owner: Direct},
		{Name: "repository-config", Owner: Chezmoi, Terminal: true}, {Name: "antidote", Owner: Chezmoi, Terminal: true},
		{Name: "tpm", Owner: Chezmoi, Terminal: true}, {Name: "obsidian.nvim", Owner: Chezmoi},
		{Name: "neovim-plugins", Owner: Chezmoi}, {Name: "ohmyzsh", Owner: Chezmoi, Terminal: true},
		{Name: "powerlevel10k", Owner: Chezmoi, Terminal: true}, {Name: "zsh-z", Owner: Chezmoi, Terminal: true},
		{Name: "zsh-autosuggestions", Owner: Chezmoi, Terminal: true}, {Name: "zsh-completions", Owner: Chezmoi, Terminal: true},
		{Name: "zsh-syntax-highlighting", Owner: Chezmoi, Terminal: true}, {Name: "zsh-history-substring-search", Owner: Chezmoi, Terminal: true},
		{Name: "tmux-sensible", Owner: Chezmoi, Terminal: true}, {Name: "tmux-resurrect", Owner: Chezmoi, Terminal: true},
		{Name: "ai-config", Owner: Chezmoi},
	}
}

// Validate rejects duplicate ownership and missing required pins.
func Validate() error {
	seen := map[string]Backend{}
	for _, item := range Items() {
		if previous, ok := seen[item.Name]; ok {
			return fmt.Errorf("%s has owners %s and %s", item.Name, previous, item.Owner)
		}
		seen[item.Name] = item.Owner
		if (item.Owner == NPM || item.Owner == Aqua || item.Owner == Direct) && item.Version == "" {
			return fmt.Errorf("%s is not pinned", item.Name)
		}
	}
	return nil
}
