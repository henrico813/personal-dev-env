// Package profile defines installer component sets.
package profile

import (
	"bytes"
	_ "embed"
	"fmt"
	"slices"

	"go.yaml.in/yaml/v3"
)

type Profile string

// Lists the profiles for error and setup messages.
const ValidValues = "terminal, client, or full"

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
	_, ok := profiles.Profiles[selected]
	return ok
}

// A group of manifest items. A profile installs each item whose tier it lists
// in profiles.yaml.
type Tier string

const (
	// Shell, tmux and command-line tools that every profile gets.
	Terminal Tier = "terminal"
	// Tools for a machine you work at, such as Neovim, OpenCode, herdr, fonts
	// and terminal app config.
	Workstation Tier = "workstation"
	// Go, Rust, the local builds and the compilers they need.
	Development Tier = "development"
)

var knownTiers = []Tier{Terminal, Workstation, Development}

//go:embed profiles.yaml
var profilesYAML []byte

// A bad embedded file stops every command at startup instead of installing
// the wrong set.
var profiles = loadOrPanic(profilesYAML)

type catalog struct {
	AptGroups map[string][]string `yaml:"apt_groups"`
	Profiles  map[Profile]entry   `yaml:"profiles"`
}

type entry struct {
	Apt   []string `yaml:"apt"`
	Tiers []Tier   `yaml:"tiers"`
}

// Reports whether profiles.yaml lists the tier for this profile. An unknown
// profile has no tiers.
func (selected Profile) Has(tier Tier) bool {
	return slices.Contains(profiles.Profiles[selected].Tiers, tier)
}

// Returns the profile's tiers in profiles.yaml order. An unknown profile gets
// an empty list rather than nil, so the chezmoi data holds [] instead of null.
func (selected Profile) Tiers() []Tier {
	return append([]Tier{}, profiles.Profiles[selected].Tiers...)
}

// Returns the sorted Ubuntu packages of the profile's apt groups in
// profiles.yaml. Each package appears once.
func (selected Profile) Packages() []string {
	var packages []string
	for _, group := range profiles.Profiles[selected].Apt {
		packages = append(packages, profiles.AptGroups[group]...)
	}
	slices.Sort(packages)
	return slices.Compact(packages)
}

// Returns the Aqua config and checksums file names. Only profiles with the
// development tier in profiles.yaml get the language servers in the larger set.
func (selected Profile) AquaFiles() (string, string) {
	if selected.Has(Development) {
		return "aqua.yaml", "aqua-checksums.json"
	}
	return "aqua-terminal.yaml", "aqua-terminal-checksums.json"
}

func load(data []byte) (catalog, error) {
	decoder := yaml.NewDecoder(bytes.NewReader(data))
	decoder.KnownFields(true)
	var loaded catalog
	if err := decoder.Decode(&loaded); err != nil {
		return catalog{}, fmt.Errorf("read profiles.yaml: %w", err)
	}
	for name, entry := range loaded.Profiles {
		for _, group := range entry.Apt {
			if _, ok := loaded.AptGroups[group]; !ok {
				return catalog{}, fmt.Errorf("profile %s uses unknown apt group %q", name, group)
			}
		}
		for _, tier := range entry.Tiers {
			if !slices.Contains(knownTiers, tier) {
				return catalog{}, fmt.Errorf("profile %s has unknown tier %q", name, tier)
			}
		}
	}
	return loaded, nil
}

func loadOrPanic(data []byte) catalog {
	loaded, err := load(data)
	if err != nil {
		panic(err)
	}
	return loaded
}
