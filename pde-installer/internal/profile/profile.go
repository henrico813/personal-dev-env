// Package profile defines installer component sets.
package profile

import "fmt"

type Profile string

const (
	Full     Profile = "full"
	Terminal Profile = "terminal"
)

func Parse(value string) (Profile, error) {
	selected := Profile(value)
	if !selected.Valid() {
		return "", fmt.Errorf("invalid profile %q; use full or terminal", value)
	}
	return selected, nil
}

// Valid reports whether the profile selects a supported component set.
func (selected Profile) Valid() bool {
	return selected == Full || selected == Terminal
}

func (selected Profile) AquaFiles() (string, string) {
	if selected == Terminal {
		return "aqua-terminal.yaml", "aqua-terminal-checksums.json"
	}
	return "aqua.yaml", "aqua-checksums.json"
}
