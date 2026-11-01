package installer

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"pde-installer/internal/profile"
)

type profileMode uint8

const (
	installProfile profileMode = iota + 1
	readProfile
)

// Installs from before profiles existed were full, so a command that only
// reads the profile, with nothing saved, and a config from that time both
// mean full.
const legacyProfile profile.Profile = "full"

// Returns the profile named by install's optional argument, or the saved
// profile when there is none.
func resolveInstallArgument(home string, args []string) (profile.Profile, error) {
	requested := ""
	if len(args) == 1 {
		requested = args[0]
	}
	if profile.Desktop(requested).Valid() {
		// A desktop argument keeps the saved profile. With no saved profile,
		// install stops instead of guessing one.
		saved, found, err := loadProfile(home)
		if err != nil || found {
			return saved, err
		}
		return "", fmt.Errorf(
			"no saved profile; first run pde-installer install %s, then pde-installer install %s",
			profile.ValidValues,
			requested,
		)
	}
	if requested != "" && !profile.Profile(requested).Valid() {
		return "", fmt.Errorf(
			"invalid install argument %q; use a profile (%s) or a desktop (%s)",
			requested,
			profile.ValidValues,
			profile.ValidDesktops,
		)
	}
	return resolveProfile(home, requested, installProfile)
}

// Returns the desktop named by install's optional argument, or the saved
// desktop. The result is "" when neither names one.
func resolveInstallDesktop(home string, args []string) (profile.Desktop, error) {
	if len(args) == 1 && profile.Desktop(args[0]).Valid() {
		return profile.Desktop(args[0]), nil
	}
	return loadDesktop(home)
}

func resolveProfile(home, requested string, mode profileMode) (profile.Profile, error) {
	saved, found, err := loadProfile(home)
	if err != nil {
		return "", err
	}
	if requested != "" {
		selected, err := profile.Parse(requested)
		if err != nil {
			return "", err
		}
		return selected, nil
	}
	if found {
		return saved, nil
	}
	switch mode {
	case readProfile:
		return legacyProfile, nil
	case installProfile:
		return "", fmt.Errorf("no saved profile; run pde-installer install terminal, pde-installer install client, or pde-installer install full")
	}
	return "", fmt.Errorf("invalid profile resolution mode")
}

func loadProfile(home string) (profile.Profile, bool, error) {
	directory := filepath.Join(home, ".config", "pde")
	path := filepath.Join(directory, "config.json")
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) || err == nil && len(bytes.TrimSpace(data)) == 0 {
		if _, legacyErr := os.Stat(filepath.Join(directory, "paths.env")); legacyErr == nil {
			return legacyProfile, true, nil
		} else if !os.IsNotExist(legacyErr) {
			return "", false, fmt.Errorf("inspect legacy PDE config: %w", legacyErr)
		}
		return "", false, nil
	}
	if err != nil {
		return "", false, fmt.Errorf("read PDE config: %w", err)
	}
	var values map[string]json.RawMessage
	if err := json.Unmarshal(data, &values); err != nil {
		return "", false, fmt.Errorf("read PDE config: %w", err)
	}
	raw, ok := values["profile"]
	if !ok {
		if _, legacy := values["install_path"]; legacy {
			return legacyProfile, true, nil
		}
		return "", false, repairProfileError(path)
	}
	var value string
	if err := json.Unmarshal(raw, &value); err != nil {
		return "", false, fmt.Errorf("read profile in %s: %w", path, err)
	}
	selected, err := profile.Parse(value)
	if err != nil {
		return "", false, fmt.Errorf("read profile in %s: %w", path, err)
	}
	return selected, true, nil
}

// Returns the desktop saved in config.json, or "" when the file or its
// desktop key is missing.
func loadDesktop(home string) (profile.Desktop, error) {
	path := filepath.Join(home, ".config", "pde", "config.json")
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) || err == nil && len(bytes.TrimSpace(data)) == 0 {
		return "", nil
	}
	if err != nil {
		return "", fmt.Errorf("read PDE config: %w", err)
	}
	var values struct {
		Desktop *string `json:"desktop"`
	}
	if err := json.Unmarshal(data, &values); err != nil {
		return "", fmt.Errorf("read PDE config: %w", err)
	}
	if values.Desktop == nil {
		return "", nil
	}
	selected, err := profile.ParseDesktop(*values.Desktop)
	if err != nil {
		return "", fmt.Errorf("read desktop in %s: %w", path, err)
	}
	return selected, nil
}

func repairProfileError(path string) error {
	return fmt.Errorf(
		"%s has no profile; add \"profile\" set to %s",
		path,
		profile.ValidValues,
	)
}
