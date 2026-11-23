// Package colorprofile defines visual profiles for managed terminal tools.
package colorprofile

import "fmt"

type Profile string

const (
	TokyoNight     Profile = "tokyo-night"
	EverforestDark Profile = "everforest-dark"
	GruvboxDark    Profile = "gruvbox-dark"
	ValidValues            = "tokyo-night, everforest-dark, or gruvbox-dark"
)

// Returns every supported color profile.
func List() []Profile {
	return []Profile{TokyoNight, EverforestDark, GruvboxDark}
}

func Parse(value string) (Profile, error) {
	selected := Profile(value)
	if !selected.Valid() {
		return "", fmt.Errorf("invalid color profile %q; use %s", value, ValidValues)
	}
	return selected, nil
}

// Valid reports whether the profile selects a supported visual theme.
func (selected Profile) Valid() bool {
	for _, supported := range List() {
		if selected == supported {
			return true
		}
	}
	return false
}
