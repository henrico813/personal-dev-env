package installer

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"pde-installer/internal/fsutil"
	"pde-installer/internal/run"
)

var legacyConfigKeys = map[string]string{
	"PDE_DEFAULT_VAULT": "default_vault",
	"PDE_MAIN_VAULT":    "main_vault",
	"PDE_WORK_VAULT":    "work_vault",
}

func prepareLegacyConfig(config config, runner run.Runner) (*fsutil.Journal, error) {
	if runner.DryRun {
		if err := runner.Plan("migrate legacy PDE configuration", nil); err != nil {
			return nil, err
		}
		return &fsutil.Journal{}, nil
	}
	return migrateLegacyConfig(config)
}

func migrateLegacyConfig(config config) (*fsutil.Journal, error) {
	directory := filepath.Join(config.Home, ".config", "pde")
	destination := filepath.Join(directory, "config.json")
	legacy := filepath.Join(directory, "paths.env")
	values := map[string]any{"install_path": config.RepoRoot}

	data, err := os.ReadFile(destination)
	if err == nil && len(bytes.TrimSpace(data)) > 0 {
		if err := json.Unmarshal(data, &values); err != nil {
			return nil, fmt.Errorf("read PDE config: %w", err)
		}
		if values == nil {
			values = make(map[string]any)
		}
	} else if err != nil && !os.IsNotExist(err) {
		return nil, fmt.Errorf("read PDE config: %w", err)
	}
	values["install_path"] = config.RepoRoot

	if data, err := os.ReadFile(legacy); err == nil {
		for _, line := range strings.Split(string(data), "\n") {
			key, value, ok := strings.Cut(strings.TrimSpace(strings.TrimPrefix(line, "export ")), "=")
			field, managed := legacyConfigKeys[key]
			if ok && managed && values[field] == nil {
				values[field] = strings.Trim(strings.TrimSpace(value), "\"'")
			}
		}
	} else if !os.IsNotExist(err) {
		return nil, fmt.Errorf("read legacy PDE config: %w", err)
	}

	updated, err := json.MarshalIndent(values, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("encode PDE config: %w", err)
	}
	updated = append(updated, '\n')
	if err := fsutil.GuardHome(config.Home, directory, destination); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(directory, 0o755); err != nil {
		return nil, fmt.Errorf("create PDE config directory: %w", err)
	}
	journal, err := fsutil.NewJournal(fsutil.JournalConfig{Home: config.Home})
	if err != nil {
		return nil, err
	}
	if !bytes.Equal(data, updated) {
		stage, err := os.CreateTemp(directory, ".config-")
		if err != nil {
			return nil, fmt.Errorf("stage PDE config: %w", err)
		}
		stagePath := stage.Name()
		removeStage := true
		defer func() {
			if removeStage {
				_ = os.Remove(stagePath)
			}
		}()
		if _, err := stage.Write(updated); err != nil {
			_ = stage.Close()
			return nil, fmt.Errorf("write staged PDE config: %w", err)
		}
		if err := stage.Close(); err != nil {
			return nil, fmt.Errorf("close staged PDE config: %w", err)
		}
		if err := journal.Activate(stagePath, destination); err != nil {
			return nil, err
		}
		removeStage = false
	}
	if err := migrateLegacyNvim(config.Home, journal); err != nil {
		return nil, journal.Revert(err)
	}
	return journal, nil
}

// migrateLegacyNvim replaces the old Neovim config link with a real directory.
// Chezmoi needs this because it cannot safely create files inside that link.
func migrateLegacyNvim(home string, journal *fsutil.Journal) error {
	destination := filepath.Join(home, ".config", "nvim")
	info, err := os.Lstat(destination)
	if os.IsNotExist(err) || err == nil && info.Mode()&os.ModeSymlink == 0 {
		return nil
	}
	if err != nil {
		return fmt.Errorf("inspect legacy Neovim config: %w", err)
	}
	target, err := os.Readlink(destination)
	if err != nil {
		return fmt.Errorf("read legacy Neovim config: %w", err)
	}
	if !legacyNvimTarget(target) {
		return fmt.Errorf("Neovim config symlink is not a legacy PDE link: %s", target)
	}
	stage, err := os.MkdirTemp(filepath.Dir(destination), ".nvim-")
	if err != nil {
		return fmt.Errorf("stage Neovim config directory: %w", err)
	}
	if err := journal.AddCleanup(stage); err != nil {
		_ = os.RemoveAll(stage)
		return err
	}
	if err := journal.Activate(stage, destination); err != nil {
		return fmt.Errorf("replace legacy Neovim config symlink: %w", err)
	}
	return nil
}

func legacyNvimTarget(target string) bool {
	target = filepath.Clean(target)
	config := filepath.Dir(target)
	return filepath.Base(target) == "nvim" && filepath.Base(config) == "config" && filepath.Base(filepath.Dir(config)) == "pde"
}
