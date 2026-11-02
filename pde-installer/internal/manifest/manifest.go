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

// Holds a managed item's owner, requested version, tier and desktop. A
// computer installs the items whose tier its profile has, except items for a
// desktop it does not have.
type Item struct {
	Name, Version string
	Owner         Backend
	Tier          profile.Tier
	Desktop       profile.Desktop // Set when only that desktop's machines get the item.
}

// Returns the items whose tier the profile has, leaving out items for other
// desktops.
func ItemsFor(selected profile.Profile, desktop profile.Desktop) []Item {
	var result []Item
	for _, item := range Items() {
		if selected.Has(item.Tier) && (item.Desktop == "" || item.Desktop == desktop) {
			result = append(result, item)
		}
	}
	return result
}

// ByOwnerFor returns selected items assigned to one backend.
func ByOwnerFor(selected profile.Profile, owner Backend) []Item {
	var result []Item
	// Desktop items are left out. Apt installs the Ubuntu ones from
	// profiles.yaml, and BuildWallpaperPlayer builds the local ones.
	for _, item := range ItemsFor(selected, "") {
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
		{Name: "zsh", Owner: Ubuntu, Tier: profile.Terminal}, {Name: "git", Owner: Ubuntu, Tier: profile.Terminal},
		{Name: "xclip", Owner: Ubuntu, Tier: profile.Terminal}, {Name: "unzip", Owner: Ubuntu, Tier: profile.Terminal},
		{
			Name:  "fontconfig",
			Owner: Ubuntu,
			Tier:  profile.Workstation,
		},
		{Name: "i3-wm", Owner: Ubuntu, Tier: profile.Terminal, Desktop: "i3"},
		{Name: "sway", Owner: Ubuntu, Tier: profile.Terminal, Desktop: "sway"},
		{Name: "tmux", Version: "3.7b", Owner: Direct, Tier: profile.Terminal},
		{Name: "aqua", Version: "v2.60.1", Owner: Aqua, Tier: profile.Terminal},
		{Name: "fd", Version: "v8.3.1", Owner: Aqua, Tier: profile.Terminal}, {Name: "fzf", Version: "v0.74.5", Owner: Aqua, Tier: profile.Terminal},
		{Name: "ripgrep", Version: "14.1.1", Owner: Aqua, Tier: profile.Terminal}, {Name: "bat", Version: "v0.19.0", Owner: Aqua, Tier: profile.Terminal},
		{Name: "jq", Version: "jq-1.7.1", Owner: Aqua, Tier: profile.Terminal},
		{Name: "gh", Version: "v2.101.0", Owner: Aqua, Tier: profile.Terminal},
		{Name: "delta", Version: "0.19.2", Owner: Aqua, Tier: profile.Terminal},
		{Name: "wt", Version: "v0.81.0", Owner: Aqua, Tier: profile.Terminal},
		{Name: "chezmoi", Version: "v2.72.0", Owner: Aqua, Tier: profile.Terminal},
		{Name: "eza", Version: "v0.23.4", Owner: Aqua, Tier: profile.Terminal}, {Name: "zoxide", Version: "v0.9.8", Owner: Aqua, Tier: profile.Terminal},
		{Name: "bottom", Version: "0.11.4", Owner: Aqua, Tier: profile.Terminal}, {Name: "yq", Version: "v4.53.3", Owner: Aqua, Tier: profile.Terminal},
		{Name: "yazi", Version: "v25.5.31", Owner: Aqua, Tier: profile.Terminal}, {Name: "ya", Version: "v25.5.31", Owner: Aqua, Tier: profile.Terminal},
		{Name: "gopls", Version: "v0.23.0", Owner: Aqua, Tier: profile.Development},
		{Name: "lua-language-server", Version: "3.19.1", Owner: Aqua, Tier: profile.Development},
		{
			Name:    "@opencode/cli",
			Version: "2.0.26",
			Owner:   NPM,
			Tier:    profile.Workstation,
		},
		{
			Name:    "@openai/codex",
			Version: "0.157.0",
			Owner:   NPM,
			Tier:    profile.Workstation,
		},
		{
			Name:    "@earendil-works/pi-coding-agent",
			Version: "0.85.1",
			Owner:   NPM,
			Tier:    profile.Workstation,
		},
		{
			Name:    "obsidian-headless",
			Version: "0.0.14",
			Owner:   NPM,
			Tier:    profile.Workstation,
		},
		{
			Name:    "@anthropic-ai/claude-code",
			Version: "2.1.283",
			Owner:   NPM,
			Tier:    profile.Workstation,
		},
		{Name: "planner", Owner: Local, Tier: profile.Development}, {Name: "pde-gh-write", Owner: Local, Tier: profile.Development}, {Name: "opencode-inline-shim", Owner: Local, Tier: profile.Development},
		{Name: "vibe", Owner: Local, Tier: profile.Development}, {Name: "blink.cmp", Owner: Local, Tier: profile.Development},
		{Name: "pde-comment-review", Owner: Local, Tier: profile.Development},
		{Name: "pde-comment-review-next", Owner: Local, Tier: profile.Development},
		{
			Name:  "skill-lessons",
			Owner: Local,
			Tier:  profile.Workstation,
		},
		{Name: "pde-comment-checker", Owner: Local, Tier: profile.Development},
		{Name: "xwinwrap", Owner: Local, Tier: profile.Terminal, Desktop: "i3"},
		{Name: "mpvpaper", Owner: Local, Tier: profile.Terminal, Desktop: "sway"},
		{
			Name:    "FiraCode",
			Version: "v3.2.1",
			Owner:   Direct,
			Tier:    profile.Workstation,
		},
		{
			Name:    "JetBrainsMono",
			Version: "v3.2.1",
			Owner:   Direct,
			Tier:    profile.Workstation,
		},
		{
			Name:    "neovim",
			Version: "0.12.3",
			Owner:   Direct,
			Tier:    profile.Workstation,
		},
		{Name: "go", Version: "1.26.4", Owner: Direct, Tier: profile.Development},
		{Name: "rust", Version: "1.96.0", Owner: Direct, Tier: profile.Development},
		{
			Name:    "node",
			Version: "24.16.0",
			Owner:   Direct,
			Tier:    profile.Workstation,
		},
		{Name: "keychain", Version: "2.9.8", Owner: Direct, Tier: profile.Development},
		// moshi-hook has no pin, and list shows "latest" as its wanted version.
		{Name: "moshi-hook", Version: "latest", Owner: Direct, Tier: profile.Development},
		{
			Name:    "herdr",
			Version: "v0.9.1",
			Owner:   Direct,
			Tier:    profile.Workstation,
		},
		{
			Name:    "nobs",
			Version: "v0.3.3",
			Owner:   Direct,
			Tier:    profile.Workstation,
		},
		{Name: "repository-config", Owner: Chezmoi, Tier: profile.Terminal}, {Name: "antidote", Owner: Chezmoi, Tier: profile.Terminal},
		{
			Name:  "tpm",
			Owner: Chezmoi,
			Tier:  profile.Terminal,
		},
		{
			Name:  "obsidian.nvim",
			Owner: Chezmoi,
			Tier:  profile.Workstation,
		},
		{
			Name:  "neovim-plugins",
			Owner: Chezmoi,
			Tier:  profile.Workstation,
		},
		{
			Name:  "ohmyzsh",
			Owner: Chezmoi,
			Tier:  profile.Terminal,
		},
		{Name: "powerlevel10k", Owner: Chezmoi, Tier: profile.Terminal}, {Name: "zsh-z", Owner: Chezmoi, Tier: profile.Terminal},
		{Name: "zsh-autosuggestions", Owner: Chezmoi, Tier: profile.Terminal}, {Name: "zsh-completions", Owner: Chezmoi, Tier: profile.Terminal},
		{Name: "zsh-syntax-highlighting", Owner: Chezmoi, Tier: profile.Terminal}, {Name: "zsh-history-substring-search", Owner: Chezmoi, Tier: profile.Terminal},
		{Name: "tmux-sensible", Owner: Chezmoi, Tier: profile.Terminal}, {Name: "tmux-resurrect", Owner: Chezmoi, Tier: profile.Terminal},
		{
			Name:  "ai-config",
			Owner: Chezmoi,
			Tier:  profile.Workstation,
		},
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
		if item.Tier == "" {
			return fmt.Errorf("%s has no tier", item.Name)
		}
	}
	return nil
}
