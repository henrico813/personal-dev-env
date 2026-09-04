package manifest

import "fmt"

// Backend identifies the system responsible for an item.
type Backend string

// Supported ownership backends.
const (
	Pkgsrc  Backend = "pkgsrc"
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
}

// PkgsrcPackages returns package paths in reconciliation order.
func PkgsrcPackages() []string {
	var packages []string
	for _, item := range ByOwner(Pkgsrc) {
		packages = append(packages, item.Name)
	}
	return packages
}

// ByOwner returns items assigned to one backend.
func ByOwner(owner Backend) []Item {
	var owned []Item
	for _, item := range Items() {
		if item.Owner == owner {
			owned = append(owned, item)
		}
	}
	return owned
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
		{Name: "pkgtools/pkg_rolling-replace", Owner: Pkgsrc},
		{Name: "shells/zsh", Owner: Pkgsrc}, {Name: "misc/tmux", Owner: Pkgsrc},
		{Name: "editors/neovim", Owner: Pkgsrc}, {Name: "lang/go", Owner: Pkgsrc},
		{Name: "lang/rust", Owner: Pkgsrc}, {Name: "lang/nodejs24", Owner: Pkgsrc},
		{Name: "devel/git-base", Owner: Pkgsrc}, {Name: "sysutils/htop", Owner: Pkgsrc},
		{Name: "security/keychain", Owner: Pkgsrc}, {Name: "x11/xclip", Owner: Pkgsrc},
		{Name: "archivers/unzip", Owner: Pkgsrc}, {Name: "fonts/fontconfig", Owner: Pkgsrc},
		{Name: "aqua", Version: "v2.60.1", Owner: Aqua},
		{Name: "fd", Version: "v8.3.1", Owner: Aqua}, {Name: "fzf", Version: "0.36.0", Owner: Aqua},
		{Name: "ripgrep", Version: "14.1.1", Owner: Aqua}, {Name: "bat", Version: "v0.19.0", Owner: Aqua},
		{Name: "jq", Version: "jq-1.7.1", Owner: Aqua}, {Name: "chezmoi", Version: "v2.72.0", Owner: Aqua},
		{Name: "eza", Version: "v0.23.4", Owner: Aqua}, {Name: "zoxide", Version: "v0.9.8", Owner: Aqua},
		{Name: "bottom", Version: "0.11.4", Owner: Aqua}, {Name: "yq", Version: "v4.53.3", Owner: Aqua},
		{Name: "yazi", Version: "v25.5.31", Owner: Aqua}, {Name: "ya", Version: "v25.5.31", Owner: Aqua},
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
		{Name: "repository-config", Owner: Chezmoi}, {Name: "antidote", Owner: Chezmoi},
		{Name: "tpm", Owner: Chezmoi}, {Name: "obsidian.nvim", Owner: Chezmoi},
		{Name: "neovim-plugins", Owner: Chezmoi}, {Name: "ohmyzsh", Owner: Chezmoi},
		{Name: "powerlevel10k", Owner: Chezmoi}, {Name: "zsh-z", Owner: Chezmoi},
		{Name: "zsh-autosuggestions", Owner: Chezmoi}, {Name: "zsh-completions", Owner: Chezmoi},
		{Name: "zsh-syntax-highlighting", Owner: Chezmoi}, {Name: "zsh-history-substring-search", Owner: Chezmoi},
		{Name: "tmux-sensible", Owner: Chezmoi}, {Name: "tmux-resurrect", Owner: Chezmoi},
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
