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
		return profile.Full, nil
	case installProfile:
		return "", fmt.Errorf("no saved profile; run pde-installer install terminal or pde-installer install full")
	}
	return "", fmt.Errorf("invalid profile resolution mode")
}

func loadProfile(home string) (profile.Profile, bool, error) {
	directory := filepath.Join(home, ".config", "pde")
	path := filepath.Join(directory, "config.json")
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) || err == nil && len(bytes.TrimSpace(data)) == 0 {
		if _, legacyErr := os.Stat(filepath.Join(directory, "paths.env")); legacyErr == nil {
			return profile.Full, true, nil
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
			return profile.Full, true, nil
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

func repairProfileError(path string) error {
	return fmt.Errorf("%s has no profile; add \"profile\": \"full\" or \"profile\": \"terminal\"", path)
}
