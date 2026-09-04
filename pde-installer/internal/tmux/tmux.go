package tmux

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"

	"pde-installer/internal/fsutil"
	"pde-installer/internal/run"
)

const (
	version       = "3.6a"
	archiveURL    = "https://github.com/tmux/tmux/releases/download/3.6a/tmux-3.6a.tar.gz"
	archiveSHA256 = "b6d8d9c76585db8ef5fa00d4931902fa4b8cbe8166f528f44fc403961a3f3759"
)

// Manager installs the pinned tmux source release.
type Manager struct {
	Home          string
	Runner        run.Runner
	archiveURL    string
	archiveSHA256 string
}

// New returns a user-local tmux manager.
func New(home string, runner run.Runner) Manager {
	return Manager{
		Home: home, Runner: runner,
		archiveURL: archiveURL, archiveSHA256: archiveSHA256,
	}
}

// ReleaseRoot returns the versioned installation prefix.
func (m Manager) ReleaseRoot() string {
	return filepath.Join(m.Home, ".local", "share", "pde", "tmux", version)
}

// Reconcile builds and transactionally activates the pinned tmux release.
func (m Manager) Reconcile() (*fsutil.Journal, error) {
	_, status, err := m.Probe()
	if err != nil && status != "broken" {
		return nil, err
	}
	if status == "current" {
		return &fsutil.Journal{}, nil
	}
	if m.Runner.DryRun {
		if err := m.Runner.Plan("download and verify "+m.archiveURL+" sha256="+m.archiveSHA256, nil); err != nil {
			return nil, err
		}
		return &fsutil.Journal{}, m.Runner.Plan("build and atomically activate tmux "+version, nil)
	}

	root := m.ReleaseRoot()
	releases := filepath.Dir(root)
	localBin := filepath.Join(m.Home, ".local", "bin")
	if err := fsutil.GuardHome(m.Home, releases, root, localBin); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(releases, 0o755); err != nil {
		return nil, fmt.Errorf("create tmux releases directory: %w", err)
	}
	workspace, err := os.MkdirTemp(releases, ".tmux-")
	if err != nil {
		return nil, fmt.Errorf("create tmux workspace: %w", err)
	}
	tracked := false
	defer func() {
		if !tracked {
			_ = os.RemoveAll(workspace)
		}
	}()

	archive := filepath.Join(workspace, "tmux-"+version+".tar.gz")
	source := filepath.Join(workspace, "source")
	destdir := filepath.Join(workspace, "install")
	if err := fsutil.GuardHome(m.Home, workspace, archive, source, destdir); err != nil {
		return nil, err
	}
	if err := m.Runner.Retry("tmux download", 3, func() error {
		return fsutil.Download(m.archiveURL, archive, m.archiveSHA256)
	}); err != nil {
		return nil, fmt.Errorf("download tmux: %w", err)
	}
	if err := extractArchive(archive, source, "tmux-"+version); err != nil {
		return nil, fmt.Errorf("extract tmux: %w", err)
	}

	environment := m.environment()
	configure := run.Command{Name: filepath.Join(source, "configure"), Args: []string{"--prefix=" + root}, Dir: source, Env: environment}
	if err := m.Runner.Run("configure tmux", configure); err != nil {
		return nil, err
	}
	build := run.Command{Name: "make", Args: []string{"-j", strconv.Itoa(runtime.NumCPU())}, Dir: source, Env: environment}
	if err := m.Runner.Run("build tmux", build); err != nil {
		return nil, err
	}
	install := run.Command{Name: "make", Args: []string{"install", "DESTDIR=" + destdir}, Dir: source, Env: environment}
	if err := m.Runner.Run("install tmux", install); err != nil {
		return nil, err
	}
	stage := filepath.Join(destdir, strings.TrimPrefix(filepath.Clean(root), string(filepath.Separator)))
	if err := verifyExecutable(filepath.Join(stage, "bin", "tmux")); err != nil {
		return nil, err
	}

	journal, err := fsutil.NewJournal(fsutil.JournalConfig{Home: m.Home})
	if err != nil {
		return nil, err
	}
	if err := journal.AddCleanup(workspace); err != nil {
		return nil, err
	}
	if err := journal.Activate(stage, root); err != nil {
		return nil, journal.Revert(fmt.Errorf("activate tmux release: %w", err))
	}
	tracked = true
	if err := os.MkdirAll(localBin, 0o755); err != nil {
		return nil, journal.Revert(fmt.Errorf("create local bin: %w", err))
	}
	stagedLink := filepath.Join(workspace, ".tmux-link")
	if err := os.Symlink(filepath.Join(root, "bin", "tmux"), stagedLink); err != nil {
		return nil, journal.Revert(fmt.Errorf("stage tmux launcher: %w", err))
	}
	if err := journal.Activate(stagedLink, filepath.Join(localBin, "tmux")); err != nil {
		return nil, journal.Revert(fmt.Errorf("activate tmux launcher: %w", err))
	}
	if _, status, err := m.Probe(); err != nil || status != "current" {
		if err == nil {
			err = fmt.Errorf("tmux status is %s", status)
		}
		return nil, journal.Revert(fmt.Errorf("verify tmux: %w", err))
	}
	return journal, nil
}

// Probe reports the managed tmux version and state.
func (m Manager) Probe() (string, string, error) {
	launcher := filepath.Join(m.Home, ".local", "bin", "tmux")
	info, err := os.Lstat(launcher)
	if os.IsNotExist(err) {
		return "", "missing", nil
	}
	if err != nil {
		return "", "broken", fmt.Errorf("inspect tmux launcher: %w", err)
	}
	if info.Mode()&os.ModeSymlink == 0 {
		return "", "outdated", nil
	}
	target, err := os.Readlink(launcher)
	if err != nil {
		return "", "broken", fmt.Errorf("read tmux launcher: %w", err)
	}
	if target != filepath.Join(m.ReleaseRoot(), "bin", "tmux") {
		return "", "outdated", nil
	}
	binary := filepath.Join(m.ReleaseRoot(), "bin", "tmux")
	if err := verifyExecutable(binary); err != nil {
		if os.IsNotExist(err) {
			return "", "missing", nil
		}
		return "", "broken", err
	}
	output, err := m.Runner.Query("read tmux version", run.Command{Name: binary, Args: []string{"-V"}, Env: m.environment()})
	if err != nil {
		return "", "broken", err
	}
	installed := strings.TrimSpace(strings.SplitN(string(output), "\n", 2)[0])
	if installed == "tmux "+version {
		return version, "current", nil
	}
	return strings.TrimPrefix(installed, "tmux "), "outdated", nil
}

func (m Manager) environment() []string {
	path := filepath.Join(m.Home, ".local", "bin") + string(os.PathListSeparator) + os.Getenv("PATH")
	return []string{"PATH=" + path}
}

func verifyExecutable(path string) error {
	info, err := os.Stat(path)
	if err != nil {
		return fmt.Errorf("stat tmux executable: %w", err)
	}
	if !info.Mode().IsRegular() || info.Mode()&0o111 == 0 || info.Size() == 0 {
		return fmt.Errorf("tmux executable is invalid: %s", path)
	}
	return nil
}
