package installer

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"

	"pde-installer/internal/colorprofile"
)

func resolveColorProfile(home, requested string) (colorprofile.Profile, error) {
	if requested != "" {
		return colorprofile.Parse(requested)
	}
	saved, found, err := loadColorProfile(home)
	if err != nil {
		return "", err
	}
	if found {
		return saved, nil
	}
	return colorprofile.TokyoNight, nil
}

func loadColorProfile(home string) (colorprofile.Profile, bool, error) {
	path := filepath.Join(home, ".config", "pde", "config.json")
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) || err == nil && len(bytes.TrimSpace(data)) == 0 {
		return "", false, nil
	}
	if err != nil {
		return "", false, fmt.Errorf("read PDE config: %w", err)
	}

	var values map[string]json.RawMessage
	if err := json.Unmarshal(data, &values); err != nil {
		return "", false, fmt.Errorf("read PDE config: %w", err)
	}
	raw, ok := values["color_profile"]
	if !ok {
		return "", false, nil
	}
	var value string
	if err := json.Unmarshal(raw, &value); err != nil {
		return "", false, fmt.Errorf("read color profile in %s: %w", path, err)
	}
	selected, err := colorprofile.Parse(value)
	if err != nil {
		return "", false, fmt.Errorf("read color profile in %s: %w", path, err)
	}
	return selected, true, nil
}
