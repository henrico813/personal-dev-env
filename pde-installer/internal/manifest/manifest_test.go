package manifest

import "testing"

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
	for _, name := range []string{"ya", "gopls", "lua-language-server"} {
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

func TestManifestIncludesSystemTools(t *testing.T) {
	t.Parallel()
	for _, name := range []string{"zsh", "git", "xclip", "unzip", "fontconfig"} {
		if _, ok := Find(name, Ubuntu); !ok {
			t.Errorf("Find(%q, Ubuntu) missing", name)
		}
	}
	item, ok := Find("tmux", Source)
	if !ok || item.Version != "3.6a" {
		t.Errorf("Find(tmux, Source) = %#v, %t", item, ok)
	}
}
