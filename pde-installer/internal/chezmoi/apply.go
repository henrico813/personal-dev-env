package chezmoi

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"pde-installer/internal/fsutil"
	"pde-installer/internal/run"
)

// Manager applies one repository's chezmoi source.
type Manager struct {
	Home, RepoRoot, AquaRoot string
	Runner                   run.Runner
}

// New returns a chezmoi manager for the supplied installation roots.
func New(home, repoRoot, aquaRoot string, runner run.Runner) Manager {
	return Manager{Home: home, RepoRoot: repoRoot, AquaRoot: aquaRoot, Runner: runner}
}

// Source returns the repository's chezmoi source directory.
func (m Manager) Source() string { return filepath.Join(m.RepoRoot, "chezmoi") }

// Apply validates and applies the managed home configuration.
func (m Manager) Apply() (*fsutil.Journal, error) {
	if err := m.Validate(); err != nil {
		return nil, err
	}
	binary := filepath.Join(m.AquaRoot, "bin", "chezmoi")
	if !m.Runner.DryRun || m.Runner.ReadOnlyDryRun {
		var err error
		binary, err = m.readOnlyBinary()
		if err != nil {
			return nil, err
		}
	}
	if _, err := os.Stat(binary); err != nil && (!m.Runner.DryRun || m.Runner.ReadOnlyDryRun) {
		return nil, fmt.Errorf("aqua-owned chezmoi not found at %s; run install first", binary)
	}
	if m.Runner.DryRun {
		arguments := m.arguments()
		arguments[9] = ""
		statusCommand := run.Command{Name: binary, Args: append(arguments, "--refresh-externals=never", "status", "--exclude", "externals,scripts", "--path-style", "absolute"), Env: m.environment()}
		diffCommand := run.Command{Name: binary, Args: append(arguments, "--refresh-externals=never", "diff", "--exclude", "externals,scripts", "--no-pager"), Env: m.environment()}
		if !m.Runner.ReadOnlyDryRun {
			if err := m.Runner.Plan("preview chezmoi changes", &diffCommand); err != nil {
				return nil, err
			}
			return &fsutil.Journal{}, nil
		}
		status, err := m.Runner.Query("read-only chezmoi status", statusCommand)
		if err != nil {
			return nil, err
		}
		diff, err := m.Runner.Query("read-only chezmoi diff", diffCommand)
		if err != nil {
			return nil, err
		}
		if _, err := fmt.Fprintln(m.Runner.Out(), "DRY-RUN: chezmoi status"); err != nil {
			return nil, fmt.Errorf("write chezmoi status heading: %w", err)
		}
		if _, err := m.Runner.Out().Write(status); err != nil {
			return nil, fmt.Errorf("write chezmoi status: %w", err)
		}
		if _, err := fmt.Fprintln(m.Runner.Out(), "DRY-RUN: chezmoi diff"); err != nil {
			return nil, fmt.Errorf("write chezmoi diff heading: %w", err)
		}
		if _, err := m.Runner.Out().Write(diff); err != nil {
			return nil, fmt.Errorf("write chezmoi diff: %w", err)
		}
		return &fsutil.Journal{}, nil
	}
	statePath := filepath.Join(m.Home, ".local", "state", "pde", "chezmoi.boltdb")
	if err := fsutil.GuardHome(m.Home, statePath, m.AquaRoot); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(filepath.Dir(statePath), 0o755); err != nil {
		return nil, fmt.Errorf("create chezmoi state directory: %w", err)
	}
	journal, err := fsutil.NewJournal(fsutil.JournalConfig{Home: m.Home})
	if err != nil {
		return nil, err
	}
	stateBackup := fsutil.BackupName(statePath)
	if _, err := os.Lstat(statePath); err == nil {
		if err := fsutil.CopyPath(statePath, stateBackup); err != nil {
			return nil, fmt.Errorf("back up chezmoi state: %w", err)
		}
		if err := journal.RecordReplaced(statePath, stateBackup); err != nil {
			return nil, journal.Revert(err)
		}
	} else if os.IsNotExist(err) {
		if err := journal.RecordCreated(statePath); err != nil {
			return nil, journal.Revert(err)
		}
	} else {
		return nil, fmt.Errorf("inspect chezmoi state: %w", err)
	}
	status, err := m.Runner.Query("preflight chezmoi status", run.Command{Name: binary, Args: append(m.arguments(), "status", "--path-style", "absolute"), Env: m.environment()})
	if err != nil {
		return nil, journal.Revert(err)
	}
	paths, err := m.changedPaths(string(status))
	if err != nil {
		return nil, journal.Revert(err)
	}
	for _, path := range paths {
		if err := fsutil.GuardHomeAllowLeafSymlink(m.Home, path); err != nil {
			return nil, journal.Revert(err)
		}
		info, err := os.Lstat(path)
		if os.IsNotExist(err) {
			if err := journal.RecordCreated(path); err != nil {
				return nil, journal.Revert(err)
			}
			continue
		} else if err != nil {
			return nil, journal.Revert(err)
		}
		name := fsutil.BackupName(path)
		if err := fsutil.GuardHome(m.Home, name); err != nil {
			return nil, journal.Revert(err)
		}
		err = fsutil.CopyPath(path, name)
		if err != nil {
			return nil, journal.Revert(fmt.Errorf("back up chezmoi target %s: %w", path, err))
		}
		if err := journal.RecordReplaced(path, name); err != nil {
			return nil, journal.Revert(err)
		}
		if info.Mode()&os.ModeSymlink != 0 {
			if err := os.Remove(path); err != nil {
				return nil, journal.Revert(fmt.Errorf("remove chezmoi target symlink %s: %w", path, err))
			}
		}
	}
	if err := fsutil.GuardHomeAllowLeafSymlink(m.Home, paths...); err != nil {
		return nil, journal.Revert(err)
	}
	if err := fsutil.GuardHome(m.Home, statePath, m.AquaRoot); err != nil {
		return nil, journal.Revert(err)
	}
	command := run.Command{Name: binary, Args: append(m.arguments(), "apply", "--no-tty"), Env: m.environment()}
	if err := m.Runner.Run("apply repository chezmoi source", command); err != nil {
		return nil, journal.Revert(err)
	}
	remaining, err := m.Runner.Query("verify chezmoi state", run.Command{Name: binary, Args: append(m.arguments(), "status", "--path-style", "absolute"), Env: m.environment()})
	if err != nil {
		return nil, journal.Revert(err)
	}
	if strings.TrimSpace(string(remaining)) != "" {
		return nil, journal.Revert(fmt.Errorf("chezmoi apply left managed drift:\n%s", remaining))
	}
	return journal, nil
}

// Status reports whether the managed home configuration has drifted.
func (m Manager) Status() string {
	status, err := m.Probe()
	if err != nil {
		return "unavailable"
	}
	return status
}

// Probe reports configuration drift without hiding command failures.
func (m Manager) Probe() (string, error) {
	proxy := filepath.Join(m.AquaRoot, "bin", "chezmoi")
	if _, err := os.Lstat(proxy); err != nil {
		if os.IsNotExist(err) {
			return "unavailable", nil
		}
		return "", fmt.Errorf("inspect chezmoi executable: %w", err)
	}
	binary, err := m.readOnlyBinary()
	if err != nil {
		return "", err
	}
	output, err := m.Runner.Query("read chezmoi status", run.Command{Name: binary, Args: append(m.arguments(), "status", "--exclude", "externals,scripts"), Env: m.environment()})
	if err != nil {
		return "", err
	}
	if strings.TrimSpace(string(output)) == "" {
		return "current", nil
	}
	return "drifted", nil
}

func (m Manager) readOnlyBinary() (string, error) {
	proxy := filepath.Join(m.AquaRoot, "bin", "chezmoi")
	info, err := os.Lstat(proxy)
	if err != nil {
		return "", fmt.Errorf("aqua-owned chezmoi not found at %s; run install first", proxy)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		return proxy, nil
	}
	root := filepath.Join(m.AquaRoot, "pkgs")
	wanted := string(filepath.Separator) + "twpayne" + string(filepath.Separator) + "chezmoi" + string(filepath.Separator) + "v2.72.0" + string(filepath.Separator)
	var matches []string
	err = filepath.WalkDir(root, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.Type().IsRegular() && entry.Name() == "chezmoi" && strings.Contains(path, wanted) {
			matches = append(matches, path)
		}
		return nil
	})
	if err != nil {
		return "", fmt.Errorf("locate pinned chezmoi executable: %w", err)
	}
	if len(matches) != 1 {
		return "", fmt.Errorf("found %d pinned chezmoi executables under %s, want 1", len(matches), root)
	}
	return matches[0], nil
}

func (m Manager) arguments() []string {
	return []string{"--config", "/dev/null", "--config-format", "toml", "--source", m.Source(), "--destination", m.Home, "--persistent-state", filepath.Join(m.Home, ".local", "state", "pde", "chezmoi.boltdb"), "--color", "false"}
}

func (m Manager) environment() []string {
	state := filepath.Join(m.Home, ".local", "state")
	if configured := os.Getenv("XDG_STATE_HOME"); filepath.IsAbs(configured) {
		state = configured
	}
	path := filepath.Join(m.AquaRoot, "bin") + string(os.PathListSeparator) + os.Getenv("PATH")
	aquaConfig := filepath.Join(m.Source(), "dot_config", "aquaproj-aqua")
	return []string{
		"AQUA_ROOT_DIR=" + m.AquaRoot,
		"AQUA_GLOBAL_CONFIG=" + filepath.Join(aquaConfig, "aqua.yaml"),
		"AQUA_CHECKSUMS_PATH=" + filepath.Join(aquaConfig, "aqua-checksums.json"),
		"PDE_SURVEIL_STATE_PATTERN=" + filepath.Join(state, "surveil", "**"),
		"PDE_REPO_ROOT=" + m.RepoRoot,
		"PATH=" + path,
	}
}

// Validate checks that the chezmoi source is complete and pinned.
func (m Manager) Validate() error {
	for _, path := range []string{m.Source(), filepath.Join(m.Source(), ".chezmoiexternal.toml"), filepath.Join(m.Source(), "dot_config", "opencode", "modify_opencode.json")} {
		if _, err := os.Stat(path); err != nil {
			return fmt.Errorf("invalid chezmoi source %s: %w", path, err)
		}
	}
	data, err := os.ReadFile(filepath.Join(m.Source(), ".chezmoiexternal.toml"))
	if err != nil {
		return err
	}
	text := string(data)
	if strings.Count(text, "type = ") != strings.Count(text, "checksum.sha256") {
		return fmt.Errorf("every chezmoi external must have a SHA256 checksum")
	}
	return nil
}

func (m Manager) changedPaths(status string) ([]string, error) {
	var paths []string
	for _, line := range strings.Split(status, "\n") {
		fields := strings.Fields(line)
		if len(fields) == 0 {
			continue
		}
		path := fields[len(fields)-1]
		if len(line) > 3 {
			path = strings.TrimSpace(line[3:])
		}
		if !filepath.IsAbs(path) {
			path = filepath.Join(m.Home, path)
		}
		clean := filepath.Clean(path)
		relative, err := filepath.Rel(m.Home, clean)
		if err != nil || relative == ".." || strings.HasPrefix(relative, ".."+string(filepath.Separator)) {
			return nil, fmt.Errorf("chezmoi target outside HOME: %s", clean)
		}
		root, err := m.mutationRoot(clean)
		if err != nil {
			return nil, err
		}
		paths = append(paths, root)
	}
	sort.Slice(paths, func(i, j int) bool { return len(paths[i]) < len(paths[j]) })
	var collapsed []string
	for _, path := range paths {
		covered := false
		for _, parent := range collapsed {
			if path == parent || strings.HasPrefix(path, parent+string(filepath.Separator)) {
				covered = true
				break
			}
		}
		if !covered {
			collapsed = append(collapsed, path)
		}
	}
	return collapsed, nil
}

func (m Manager) mutationRoot(path string) (string, error) {
	relative, _ := filepath.Rel(m.Home, path)
	current := m.Home
	firstMissing := ""
	for _, component := range strings.Split(relative, string(filepath.Separator)) {
		current = filepath.Join(current, component)
		info, err := os.Lstat(current)
		if os.IsNotExist(err) {
			if firstMissing == "" {
				firstMissing = current
			}
			continue
		}
		if err != nil {
			return "", fmt.Errorf("inspect chezmoi target %s: %w", current, err)
		}
		if info.Mode()&os.ModeSymlink != 0 {
			if current != path {
				return "", fmt.Errorf("chezmoi target has symlink ancestor: %s", current)
			}
			return current, nil
		}
	}
	if firstMissing != "" {
		return firstMissing, nil
	}
	return path, nil
}
