package pkgsrc

import (
	"crypto/sha256"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
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

func (m Manager) bootstrapStampPath() string {
	return filepath.Join(m.StateDir, bootstrapStamp)
}

func (m Manager) sourceDigestPath() string {
	return filepath.Join(m.StateDir, "pkgsrc-source-"+release+".sha256")
}

func (m Manager) workRoot() string {
	return filepath.Join(m.StateDir, "pkgsrc-work", release)
}

func (m Manager) distDir() string {
	return filepath.Join(m.StateDir, "pkgsrc-distfiles", release)
}

func (m Manager) packageRoot() string {
	return filepath.Join(m.StateDir, "pkgsrc-packages", release)
}

// Bootstrap creates the pinned unprivileged pkgsrc installation.
func (m Manager) Bootstrap() error {
	if executable(m.Bmake()) && executable(m.pkgInfo()) && m.ValidateBootstrap() == nil {
		return nil
	}
	sourceCurrent := m.ValidateExtractedSource() == nil
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
		if err := m.ensureArchive(archive); err != nil {
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
		if err := m.validateSourceLayout(); err != nil {
			return fmt.Errorf("validate extracted pkgsrc: %w", err)
		}
		if err := m.writeSourceDigest(); err != nil {
			return fmt.Errorf("record extracted pkgsrc digest: %w", err)
		}
		if err := m.ValidateExtractedSource(); err != nil {
			return fmt.Errorf("validate extracted pkgsrc digest: %w", err)
		}
	}
	command := run.Command{Name: filepath.Join(m.SourceRoot, "bootstrap", "bootstrap"), Args: m.bootstrapArgs(), Dir: filepath.Join(m.SourceRoot, "bootstrap")}
	if err := fsutil.GuardHome(m.Home, m.SourceRoot, m.Prefix, m.StateDir, m.workRoot(), m.distDir(), m.packageRoot()); err != nil {
		return err
	}
	if err := os.MkdirAll(m.workRoot(), 0o755); err != nil {
		return fmt.Errorf("create pkgsrc work root: %w", err)
	}
	if err := os.MkdirAll(m.distDir(), 0o755); err != nil {
		return fmt.Errorf("create pkgsrc distfiles: %w", err)
	}
	if err := os.MkdirAll(m.packageRoot(), 0o755); err != nil {
		return fmt.Errorf("create pkgsrc package root: %w", err)
	}
	if err := m.Runner.Run("bootstrap unprivileged pkgsrc", command); err != nil {
		return err
	}
	if err := m.ValidateExtractedSource(); err != nil {
		return fmt.Errorf("pkgsrc source changed during bootstrap: %w", err)
	}
	if err := fsutil.GuardHome(m.Home, m.StateDir, m.bootstrapStampPath()); err != nil {
		return err
	}
	if err := os.WriteFile(m.bootstrapStampPath(), []byte(archiveSHA256+"\n"), 0o644); err != nil {
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
		if err := m.ValidateExtractedSource(); err != nil {
			return fmt.Errorf("validate pkgsrc source before reconcile: %w", err)
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
		post, err := m.packageStatus(packagePath)
		if err != nil {
			return fmt.Errorf("verify %s after %s: %w", packagePath, target, err)
		}
		if post.Installed != status.Requested {
			return fmt.Errorf("verify %s after %s: installed %q, want exactly %q", packagePath, target, post.Installed, status.Requested)
		}
		if err := m.ValidateExtractedSource(); err != nil {
			return fmt.Errorf("pkgsrc source changed during %s: %w", target, err)
		}
	}
	if m.Runner.DryRun {
		return nil
	}
	return m.writeTreeState(wanted)
}

// Statuses returns the state of each requested package.
func (m Manager) Statuses() ([]Status, error) {
	packages := manifest.PkgsrcPackages()
	statuses := make([]Status, 0, len(packages))
	if !executable(m.Bmake()) || !executable(m.pkgInfo()) {
		for _, packagePath := range packages {
			statuses = append(statuses, Status{Path: packagePath, Requested: "pkgsrc-" + release, State: "not-bootstrapped"})
		}
		return statuses, nil
	}
	for _, packagePath := range packages {
		status, err := m.packageStatus(packagePath)
		if err != nil {
			return nil, fmt.Errorf("status %s: %w", packagePath, err)
		}
		statuses = append(statuses, status)
	}
	return statuses, nil
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
	} else if !isPackageMissing(err) {
		return Status{}, err
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
	if err := fsutil.GuardHome(m.Home, m.SourceRoot, m.Prefix, m.StateDir, m.workRoot(), m.distDir(), m.packageRoot()); err != nil {
		return err
	}
	if err := os.MkdirAll(m.workRoot(), 0o755); err != nil {
		return fmt.Errorf("create pkgsrc work root: %w", err)
	}
	if err := os.MkdirAll(m.distDir(), 0o755); err != nil {
		return fmt.Errorf("create pkgsrc distfiles: %w", err)
	}
	if err := os.MkdirAll(m.packageRoot(), 0o755); err != nil {
		return fmt.Errorf("create pkgsrc package root: %w", err)
	}
	backup, err := m.backupDatabase(directory)
	if err != nil {
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
}

func isPackageMissing(err error) bool {
	var exitError *exec.ExitError
	return errors.As(err, &exitError) && exitError.ExitCode() == 1
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
	return []string{"--unprivileged", "--prefix", m.Prefix, "--pkgdbdir", m.pkgDB(), "--workdir", filepath.Join(m.workRoot(), "bootstrap")}
}

func (m Manager) makeVariables() []string {
	return []string{
		"LOCALBASE=" + m.Prefix,
		"PREFIX=" + m.Prefix,
		"PKG_DBDIR=" + m.pkgDB(),
		"WRKOBJDIR=" + filepath.Join(m.workRoot(), "packages"),
		"DISTDIR=" + m.distDir(),
		"PACKAGES=" + m.packageRoot(),
	}
}

func executable(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Mode().IsRegular() && info.Mode()&0o111 != 0
}

func (m Manager) stampCurrent() bool {
	data, err := os.ReadFile(m.bootstrapStampPath())
	return err == nil && strings.TrimSpace(string(data)) == archiveSHA256
}

// ValidateExtractedSource checks integrity so changed source is never reused.
func (m Manager) ValidateExtractedSource() error {
	if err := m.validateSourceLayout(); err != nil {
		return err
	}
	expected, err := os.ReadFile(m.sourceDigestPath())
	if err != nil {
		return fmt.Errorf("read pkgsrc source digest: %w", err)
	}
	actual, err := sourceDigest(m.SourceRoot)
	if err != nil {
		return fmt.Errorf("hash pkgsrc source: %w", err)
	}
	if strings.TrimSpace(string(expected)) != actual {
		return fmt.Errorf("pkgsrc source digest changed: got %s, want %s", actual, strings.TrimSpace(string(expected)))
	}
	return nil
}

// ValidateBootstrap requires both checks before reusing an installation.
func (m Manager) ValidateBootstrap() error {
	if err := m.ValidateExtractedSource(); err != nil {
		return err
	}
	if !m.stampCurrent() {
		return fmt.Errorf("pkgsrc bootstrap stamp does not match %s", archiveSHA256)
	}
	return nil
}

func (m Manager) validateSourceLayout() error {
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
	return nil
}

func (m Manager) ensureArchive(archive string) error {
	if digest, err := fsutil.FileSHA256(archive); err == nil && strings.EqualFold(digest, archiveSHA256) {
		return nil
	}
	return m.Runner.Retry("pkgsrc download", 3, func() error {
		if err := fsutil.GuardHome(m.Home, filepath.Dir(archive), archive); err != nil {
			return err
		}
		return fsutil.Download(archiveURL, archive, archiveSHA256)
	})
}

func (m Manager) writeSourceDigest() error {
	digest, err := sourceDigest(m.SourceRoot)
	if err != nil {
		return err
	}
	if err := fsutil.GuardHome(m.Home, m.StateDir, m.sourceDigestPath()); err != nil {
		return err
	}
	if err := os.MkdirAll(m.StateDir, 0o755); err != nil {
		return err
	}
	return os.WriteFile(m.sourceDigestPath(), []byte(digest+"\n"), 0o644)
}

func sourceDigest(root string) (string, error) {
	hash := sha256.New()
	err := filepath.WalkDir(root, func(path string, entry os.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		relative, err := filepath.Rel(root, path)
		if err != nil {
			return err
		}
		info, err := entry.Info()
		if err != nil {
			return err
		}
		if _, err := fmt.Fprintf(hash, "%d:%s:%s\x00", len(relative), filepath.ToSlash(relative), info.Mode().String()); err != nil {
			return err
		}
		if info.Mode()&os.ModeSymlink != 0 {
			target, err := os.Readlink(path)
			if err != nil {
				return err
			}
			_, err = fmt.Fprintf(hash, "%d:%s\x00", len(target), target)
			return err
		}
		if !info.Mode().IsRegular() {
			return nil
		}
		file, err := os.Open(path)
		if err != nil {
			return err
		}
		_, copyErr := io.Copy(hash, file)
		closeErr := file.Close()
		return errors.Join(copyErr, closeErr)
	})
	if err != nil {
		return "", err
	}
	return fmt.Sprintf("%x", hash.Sum(nil)), nil
}
