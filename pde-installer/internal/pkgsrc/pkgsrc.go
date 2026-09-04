package pkgsrc

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"pde-installer/internal/fsutil"
	"pde-installer/internal/manifest"
	"pde-installer/internal/run"
)

const (
	release        = "2026Q2"
	archiveURL     = "https://cdn.NetBSD.org/pub/pkgsrc/pkgsrc-2026Q2/pkgsrc-2026Q2.tar.xz"
	archiveSHA256  = "61d160d3c345d4a36bdecb2c1db034256974acca5ec5a5075e80c7ca5da916aa"
	bootstrapStamp = ".pde-bootstrap-2026Q2"
)

// Manager reconciles one unprivileged pkgsrc installation.
type Manager struct {
	Home, SourceRoot, Prefix, StateDir string
	Runner                             run.Runner
}

// Status describes the requested and installed package state.
type Status struct {
	Path, Requested, Installed, State string
}

type treeState struct {
	Release       string   `json:"release"`
	ArchiveSHA256 string   `json:"archive_sha256"`
	Packages      []string `json:"packages"`
}

// New returns a pkgsrc manager rooted in the user's home directory.
func New(home string, runner run.Runner) Manager {
	return Manager{
		Home: home, SourceRoot: filepath.Join(home, ".local", "src", "pkgsrc-"+release),
		Prefix: filepath.Join(home, ".local", "pkg"), StateDir: filepath.Join(home, ".local", "state", "pde"),
		Runner: runner,
	}
}

// Bmake returns the managed bmake executable path.
func (m Manager) Bmake() string   { return filepath.Join(m.Prefix, "bin", "bmake") }
func (m Manager) pkgInfo() string { return filepath.Join(m.Prefix, "sbin", "pkg_info") }
func (m Manager) pkgDB() string   { return filepath.Join(m.Prefix, "pkgdb") }

// Bootstrap creates the pinned unprivileged pkgsrc installation.
func (m Manager) Bootstrap() error {
	if executable(m.Bmake()) && executable(m.pkgInfo()) && m.ValidateSource(true) == nil {
		return nil
	}
	sourceCurrent := m.ValidateSource(false) == nil
	archive := filepath.Join(m.StateDir, "downloads", "pkgsrc-"+release+".tar.xz")
	if m.Runner.DryRun {
		if !sourceCurrent {
			if err := m.Runner.Plan("download and verify "+archiveURL+" sha256="+archiveSHA256, nil); err != nil {
				return err
			}
			if err := m.Runner.Plan("extract pkgsrc directly into "+m.SourceRoot, nil); err != nil {
				return err
			}
		}
		return m.Runner.Run("bootstrap unprivileged pkgsrc", run.Command{Name: filepath.Join(m.SourceRoot, "bootstrap", "bootstrap"), Args: m.bootstrapArgs(), Dir: filepath.Join(m.SourceRoot, "bootstrap")})
	}
	if err := fsutil.GuardHome(m.Home, m.StateDir, filepath.Dir(archive), archive, m.SourceRoot, m.Prefix); err != nil {
		return err
	}
	if !sourceCurrent {
		if err := m.Runner.Retry("pkgsrc download", 3, func() error {
			if err := fsutil.GuardHome(m.Home, filepath.Dir(archive), archive); err != nil {
				return err
			}
			return fsutil.Download(archiveURL, archive, archiveSHA256)
		}); err != nil {
			return err
		}
		if err := fsutil.GuardHome(m.Home, m.SourceRoot); err != nil {
			return err
		}
		if err := os.RemoveAll(m.SourceRoot); err != nil {
			return fmt.Errorf("remove invalid pkgsrc source: %w", err)
		}
		if err := os.MkdirAll(m.SourceRoot, 0o755); err != nil {
			return fmt.Errorf("create pkgsrc source: %w", err)
		}
		if err := m.Runner.Run("extract pkgsrc", run.Command{Name: "tar", Args: []string{"-xJf", archive, "--strip-components=2", "-C", m.SourceRoot}}); err != nil {
			return err
		}
		if err := m.ValidateSource(false); err != nil {
			return fmt.Errorf("validate extracted pkgsrc: %w", err)
		}
	}
	command := run.Command{Name: filepath.Join(m.SourceRoot, "bootstrap", "bootstrap"), Args: m.bootstrapArgs(), Dir: filepath.Join(m.SourceRoot, "bootstrap")}
	if err := fsutil.GuardHome(m.Home, m.SourceRoot, m.Prefix); err != nil {
		return err
	}
	if err := m.Runner.Retry("pkgsrc bootstrap", 2, func() error { return m.Runner.Run("bootstrap unprivileged pkgsrc", command) }); err != nil {
		return err
	}
	if err := fsutil.GuardHome(m.Home, m.SourceRoot); err != nil {
		return err
	}
	if err := os.WriteFile(filepath.Join(m.SourceRoot, bootstrapStamp), []byte(archiveSHA256+"\n"), 0o644); err != nil {
		return fmt.Errorf("write pkgsrc bootstrap stamp: %w", err)
	}
	return nil
}

// Reconcile installs the complete requested package set.
func (m Manager) Reconcile() error {
	wanted := treeState{Release: release, ArchiveSHA256: archiveSHA256, Packages: manifest.PkgsrcPackages()}
	previous, hasPrevious := m.readTreeState()
	treeTransitioned := !hasPrevious || previous.Release != wanted.Release || previous.ArchiveSHA256 != wanted.ArchiveSHA256
	desiredTransitioned := treeTransitioned || !samePackages(previous.Packages, wanted.Packages)
	if !m.Runner.DryRun {
		if err := fsutil.GuardHome(m.Home, m.SourceRoot, m.Prefix, m.StateDir); err != nil {
			return err
		}
	}
	for _, packagePath := range manifest.PkgsrcPackages() {
		if m.Runner.DryRun {
			if err := m.Runner.Plan("reconcile pkgsrc package "+packagePath, nil); err != nil {
				return err
			}
			continue
		}
		status, err := m.packageStatus(packagePath)
		if err != nil {
			return err
		}
		if status.State == "current" && !desiredTransitioned {
			continue
		}
		target := "install"
		if status.Installed != "" {
			target = "replace"
		}
		if err := m.mutate(packagePath, target, treeTransitioned); err != nil {
			return err
		}
	}
	if m.Runner.DryRun {
		return nil
	}
	return m.writeTreeState(wanted)
}

// Statuses returns the state of each requested package.
func (m Manager) Statuses() []Status {
	packages := manifest.PkgsrcPackages()
	statuses := make([]Status, 0, len(packages))
	for _, packagePath := range packages {
		status, err := m.packageStatus(packagePath)
		if err != nil {
			statuses = append(statuses, Status{Path: packagePath, Requested: "pkgsrc-" + release, State: "not-bootstrapped"})
			continue
		}
		statuses = append(statuses, status)
	}
	return statuses
}

func (m Manager) packageStatus(packagePath string) (Status, error) {
	directory := filepath.Join(m.SourceRoot, packagePath)
	requested, err := m.showVar(directory, "PKGNAME")
	if err != nil {
		return Status{}, err
	}
	base, err := m.showVar(directory, "PKGBASE")
	if err != nil {
		return Status{}, err
	}
	output, err := m.Runner.Query("query installed "+base, run.Command{Name: m.pkgInfo(), Args: []string{"-e", base + "-[0-9]*"}, Env: m.environment()})
	installed := ""
	if err == nil {
		installed = strings.TrimSpace(string(output))
	}
	state := "missing"
	if installed == requested {
		state = "current"
	} else if installed != "" {
		state = "outdated"
	}
	return Status{Path: packagePath, Requested: requested, Installed: installed, State: state}, nil
}

func (m Manager) showVar(directory, variable string) (string, error) {
	args := append(m.makeVariables(), "show-var", "VARNAME="+variable)
	output, err := m.Runner.Query("read pkgsrc "+variable, run.Command{Name: m.Bmake(), Args: args, Dir: directory, Env: m.environment()})
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(output)), nil
}

func (m Manager) mutate(packagePath, target string, treeTransitioned bool) error {
	directory := filepath.Join(m.SourceRoot, packagePath)
	command := run.Command{Name: m.Bmake(), Args: append(m.makeVariables(), target), Dir: directory, Env: m.environment()}
	return m.Runner.Retry(packagePath+" "+target, 2, func() error {
		if err := fsutil.GuardHome(m.Home, m.SourceRoot, m.Prefix, m.StateDir); err != nil {
			return err
		}
		backup, err := m.backupDatabase(directory)
		if err != nil {
			return err
		}
		if err := fsutil.GuardHome(m.Home, m.SourceRoot, m.Prefix); err != nil {
			return err
		}
		output, err := m.Runner.RunOutput(target+" "+packagePath, command)
		if err != nil {
			return fmt.Errorf("%w; package database backup retained at %s; recover with: cd %s && %s %s; inspect with: %s -a", err, backup, directory, m.Bmake(), target, m.pkgInfo())
		}
		if treeTransitioned && target == "replace" && dependencyRepairNeeded(string(output)) {
			recovery := filepath.Join(m.Prefix, "sbin", "pkg_rolling-replace")
			if !executable(recovery) {
				return fmt.Errorf("%s replace requires dependency repair but %s is unavailable", packagePath, recovery)
			}
			if err := fsutil.GuardHome(m.Home, m.SourceRoot, m.Prefix); err != nil {
				return err
			}
			if err := m.Runner.Run("repair pkgsrc reverse dependencies", run.Command{Name: recovery, Args: []string{"-u"}, Env: m.environment()}); err != nil {
				return fmt.Errorf("pkg_rolling-replace after %s: %w", packagePath, err)
			}
		}
		return nil
	})
}

func (m Manager) backupDatabase(packageDirectory string) (string, error) {
	database, err := m.showVar(packageDirectory, "PKG_DBDIR")
	if err != nil {
		return "", err
	}
	if filepath.Clean(database) != m.pkgDB() {
		return "", fmt.Errorf("pkgsrc PKG_DBDIR escaped fixed prefix: got %s, want %s", database, m.pkgDB())
	}
	backup := filepath.Join(m.StateDir, "pkgdb-backups", time.Now().UTC().Format("20060102T150405.000000000Z"))
	if err := fsutil.GuardHome(m.Home, database, backup); err != nil {
		return "", err
	}
	if err := os.MkdirAll(backup, 0o700); err != nil {
		return "", err
	}
	if _, err := os.Stat(database); os.IsNotExist(err) {
		return backup, nil
	}
	if err := fsutil.CopyTree(database, backup); err != nil {
		return "", err
	}
	return backup, nil
}

func dependencyRepairNeeded(output string) bool {
	return strings.Contains(output, "unsafe_depends") || strings.Contains(output, "unsafe_depends_strict")
}

func samePackages(left, right []string) bool {
	if len(left) != len(right) {
		return false
	}
	for index := range left {
		if left[index] != right[index] {
			return false
		}
	}
	return true
}

func (m Manager) treeStatePath() string {
	return filepath.Join(m.StateDir, "pkgsrc-tree.json")
}

func (m Manager) readTreeState() (treeState, bool) {
	data, err := os.ReadFile(m.treeStatePath())
	if err != nil {
		return treeState{}, false
	}
	var state treeState
	if json.Unmarshal(data, &state) != nil {
		return treeState{}, false
	}
	return state, true
}

func (m Manager) writeTreeState(state treeState) error {
	if err := fsutil.GuardHome(m.Home, m.StateDir, m.treeStatePath()); err != nil {
		return err
	}
	if err := os.MkdirAll(m.StateDir, 0o755); err != nil {
		return err
	}
	data, err := json.Marshal(state)
	if err != nil {
		return err
	}
	stage, err := os.CreateTemp(m.StateDir, ".pkgsrc-tree-")
	if err != nil {
		return err
	}
	name := stage.Name()
	defer func() { _ = os.Remove(name) }()
	if _, err := stage.Write(append(data, '\n')); err != nil {
		_ = stage.Close()
		return err
	}
	if err := stage.Close(); err != nil {
		return err
	}
	return os.Rename(name, m.treeStatePath())
}

func (m Manager) environment() []string {
	path := filepath.Join(m.Prefix, "bin") + string(os.PathListSeparator) + filepath.Join(m.Prefix, "sbin") + string(os.PathListSeparator) + os.Getenv("PATH")
	return []string{"PATH=" + path, "PKG_CONFIG_PATH=" + filepath.Join(m.Prefix, "lib", "pkgconfig"), "PKG_DBDIR=" + m.pkgDB()}
}

func (m Manager) bootstrapArgs() []string {
	return []string{"--unprivileged", "--prefix", m.Prefix, "--pkgdbdir", m.pkgDB(), "--workdir", filepath.Join(m.SourceRoot, "bootstrap", "work")}
}

func (m Manager) makeVariables() []string {
	return []string{"LOCALBASE=" + m.Prefix, "PREFIX=" + m.Prefix, "PKG_DBDIR=" + m.pkgDB()}
}

func executable(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Mode().IsRegular() && info.Mode()&0o111 != 0
}

func (m Manager) stampCurrent() bool {
	data, err := os.ReadFile(filepath.Join(m.SourceRoot, bootstrapStamp))
	return err == nil && strings.TrimSpace(string(data)) == archiveSHA256
}

// ValidateSource checks release metadata, package paths, and bootstrap state.
func (m Manager) ValidateSource(requireStamp bool) error {
	tag, err := os.ReadFile(filepath.Join(m.SourceRoot, "CVS", "Tag"))
	if err != nil || strings.TrimSpace(string(tag)) != "Tpkgsrc-"+release {
		return fmt.Errorf("pkgsrc source tag is not pkgsrc-%s", release)
	}
	if _, err := os.Stat(filepath.Join(m.SourceRoot, "doc", "CHANGES-pkgsrc-"+release)); err != nil {
		return fmt.Errorf("pkgsrc release metadata: %w", err)
	}
	if info, err := os.Stat(filepath.Join(m.SourceRoot, "bootstrap", "bootstrap")); err != nil || info.Mode()&0o111 == 0 {
		return fmt.Errorf("pkgsrc bootstrap script is missing or not executable")
	}
	for _, packagePath := range manifest.PkgsrcPackages() {
		if info, err := os.Stat(filepath.Join(m.SourceRoot, packagePath, "Makefile")); err != nil || !info.Mode().IsRegular() {
			return fmt.Errorf("pkgsrc package path is missing: %s", packagePath)
		}
	}
	if requireStamp && !m.stampCurrent() {
		return fmt.Errorf("pkgsrc bootstrap stamp does not match %s", archiveSHA256)
	}
	return nil
}
