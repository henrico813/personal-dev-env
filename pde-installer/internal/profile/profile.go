// Package profile defines installer component sets.
package profile

import "fmt"

type Profile string

const (
	// Installs everything for dev machines, including Go, Rust and local builds.
	Full Profile = "full"
	// Installs Neovim, OpenCode, herdr and fonts for a laptop that builds
	// nothing. It leaves out Go, Rust, local builds and the approval tools.
	Client Profile = "client"
	// Installs the shell, tmux and command-line tools for headless homelab VMs.
	Terminal    Profile = "terminal"
	ValidValues         = "terminal, client, or full"
)

func Parse(value string) (Profile, error) {
	selected := Profile(value)
	if !selected.Valid() {
		return "", fmt.Errorf(
			"invalid profile %q; use %s",
			value,
			ValidValues,
		)
	}
	return selected, nil
}

// Valid reports whether the profile selects a supported component set.
func (selected Profile) Valid() bool {
	return selected == Full || selected == Client || selected == Terminal
}

// Reports whether the profile installs tools for a machine you work at,
// such as Neovim, OpenCode, herdr, fonts and terminal app config.
func (selected Profile) HasWorkstationTools() bool {
	return selected == Client || selected == Full
}

func (selected Profile) AquaFiles() (string, string) {
	if selected == Terminal || selected == Client {
		return "aqua-terminal.yaml", "aqua-terminal-checksums.json"
	}
	return "aqua.yaml", "aqua-checksums.json"
}
