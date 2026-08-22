package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/tidwall/sjson"
)

const openCodeConfigSchema = "https://opencode.ai/config.json"

func installSurveilOpenCodePermission(cfg *Config, runner Runner) error {
	configPath := filepath.Join(cfg.OpenCodeConfigDir, "opencode.json")
	data, mode, exists, err := readOpenCodeConfig(configPath)
	if err != nil {
		return err
	}
	if !exists {
		data = []byte(fmt.Sprintf(`{"$schema":%q}`, openCodeConfigSchema))
	}

	updated, changed, err := mergeSurveilOpenCodePermission(data, surveilStatePattern(cfg.HomeDir))
	if err != nil {
		return fmt.Errorf("merge Surveil OpenCode permission: %w", err)
	}
	if !changed {
		return nil
	}

	var formatted bytes.Buffer
	if err := json.Indent(&formatted, updated, "", "  "); err != nil {
		return fmt.Errorf("format OpenCode config: %w", err)
	}
	formatted.WriteByte('\n')

	if err := runner.MkdirAll("create OpenCode config dir", cfg.OpenCodeConfigDir, 0o755); err != nil {
		return err
	}
	if exists {
		if err := backupConfigInstallPath(configPath, runner); err != nil {
			return err
		}
	}
	return runner.WriteFile("write Surveil OpenCode permission", configPath, formatted.Bytes(), mode)
}

func readOpenCodeConfig(path string) ([]byte, os.FileMode, bool, error) {
	info, err := os.Stat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return nil, 0o644, false, nil
		}
		return nil, 0, false, fmt.Errorf("stat OpenCode config: %w", err)
	}
	if !info.Mode().IsRegular() {
		return nil, 0, false, fmt.Errorf("OpenCode config is not a regular file: %s", path)
	}

	data, err := os.ReadFile(path)
	if err != nil {
		return nil, 0, false, fmt.Errorf("read OpenCode config: %w", err)
	}
	trimmed := bytes.TrimSpace(data)
	if len(trimmed) == 0 || bytes.Equal(trimmed, []byte("null")) {
		data = []byte("{}")
	}
	return data, info.Mode().Perm(), true, nil
}

func mergeSurveilOpenCodePermission(data []byte, pattern string) ([]byte, bool, error) {
	if !filepath.IsAbs(pattern) {
		return nil, false, fmt.Errorf("Surveil state pattern must be absolute: %s", pattern)
	}

	var root map[string]any
	if err := json.Unmarshal(data, &root); err != nil {
		return nil, false, fmt.Errorf("decode OpenCode config: %w", err)
	}
	if root == nil {
		root = map[string]any{}
		data = []byte("{}")
	}

	permissionValue, hasPermission := root["permission"]
	if !hasPermission {
		updated, err := sjson.SetBytes(data, permissionPath(pattern), "allow")
		return updated, true, err
	}
	if action, ok := permissionValue.(string); ok {
		if action == "allow" {
			return data, false, nil
		}
		return nil, false, fmt.Errorf("permission string %q cannot preserve a narrow exception", action)
	}

	permission, ok := permissionValue.(map[string]any)
	if !ok {
		return nil, false, fmt.Errorf("permission must be a string or object")
	}
	externalValue, hasExternal := permission["external_directory"]
	if !hasExternal {
		updated, err := sjson.SetBytes(data, permissionPath(pattern), "allow")
		return updated, true, err
	}
	if action, ok := externalValue.(string); ok {
		if action == "allow" {
			return data, false, nil
		}
		if action != "ask" && action != "deny" {
			return nil, false, fmt.Errorf("unsupported external_directory action %q", action)
		}
		rules, err := json.Marshal(map[string]string{"*": action, pattern: "allow"})
		if err != nil {
			return nil, false, err
		}
		updated, err := sjson.SetRawBytes(data, "permission.external_directory", rules)
		return updated, true, err
	}

	external, ok := externalValue.(map[string]any)
	if !ok {
		return nil, false, fmt.Errorf("external_directory must be a string or object")
	}
	if action, ok := external[pattern].(string); ok && action == "allow" {
		return data, false, nil
	}
	updated, err := sjson.SetBytes(data, permissionPath(pattern), "allow")
	return updated, true, err
}

func permissionPath(pattern string) string {
	replacer := strings.NewReplacer(
		`\`, `\\`, `.`, `\.`, `:`, `\:`, `|`, `\|`,
		`#`, `\#`, `@`, `\@`, `*`, `\*`, `?`, `\?`,
	)
	return "permission.external_directory." + replacer.Replace(pattern)
}

func surveilStatePattern(homeDir string) string {
	stateHome := os.Getenv("XDG_STATE_HOME")
	if !filepath.IsAbs(stateHome) {
		stateHome = filepath.Join(homeDir, ".local", "state")
	}
	return filepath.Join(stateHome, "surveil", "**")
}
