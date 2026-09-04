package npm

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"pde-installer/internal/fsutil"
	"pde-installer/internal/manifest"
	"pde-installer/internal/run"
)

type packageSpec struct {
	Name      string `json:"name"`
	Version   string `json:"version"`
	Binary    string `json:"binary"`
	Integrity string `json:"integrity"`
}

func packages() []packageSpec {
	specs := []packageSpec{
		{Name: "opencode-ai", Binary: "opencode", Integrity: "sha512-5xrG2gQEwV2sLus30SZX9GyLbPX3z57BCxddedDM0wx1bgnwlHVLOS/FD2uve7fEZlmkr7KYFbvs65ySz1rwzA=="},
		{Name: "@openai/codex", Binary: "codex", Integrity: "sha512-IRocJlE+jCZGYHwIJWBja2nDswTSZY4sNddQgU5xiR/mVWxo5WcO8pqcajXJea1XDoNK9CaVzizVhUxDhFkU6g=="},
		{Name: "@earendil-works/pi-coding-agent", Binary: "pi", Integrity: "sha512-jmOlrqUmvhh/siNWFRXjYLJzhKFIHNsAQaysRwzQPQFnPAaV/vhqHsLH/MBsIISA1Rjj7WTUFR3nJrpXoLx39w=="},
		{Name: "obsidian-headless", Binary: "ob", Integrity: "sha512-S1d/hxLKvCUG2g5tRyXFkzPqMs3Ntw1tDyzoF2yfHGRuB4B+Mi3X2vgT8LbfQKrkEEi3LfJRdXtYzAVHcbpccw=="},
	}
	for index := range specs {
		item, _ := manifest.Find(specs[index].Name, manifest.NPM)
		specs[index].Version = item.Version
	}
	return specs
}

// Manager installs Node.js tools for one installation.
type Manager struct {
	Home, RepoRoot string
	Runner         run.Runner
}

type lockFile struct {
	LockfileVersion int                  `json:"lockfileVersion"`
	Packages        map[string]lockEntry `json:"packages"`
}

type lockEntry struct {
	Version      string            `json:"version"`
	Resolved     string            `json:"resolved"`
	Integrity    string            `json:"integrity"`
	Dependencies map[string]string `json:"dependencies"`
	Link         bool              `json:"link"`
}

// New returns an npm tool manager.
func New(home, repoRoot string, runner run.Runner) Manager {
	return Manager{Home: home, RepoRoot: repoRoot, Runner: runner}
}

// Root returns the managed npm installation prefix.
func (m Manager) Root() string { return filepath.Join(m.Home, ".local", "share", "pde", "npm") }

// Reconcile installs and activates all pinned npm tools.
func (m Manager) Reconcile() (*fsutil.Journal, error) {
	if err := m.ValidateLock(); err != nil {
		return nil, err
	}
	npmBinary := filepath.Join(m.Home, ".local", "bin", "npm")
	if m.Runner.DryRun {
		if err := m.Runner.Plan("materialize complete npm lock with pinned npm ci", nil); err != nil {
			return nil, err
		}
		if err := m.Runner.Plan("run required npm package install scripts in staging", nil); err != nil {
			return nil, err
		}
		if err := m.Runner.Plan("atomically activate npm prefix and executables", nil); err != nil {
			return nil, err
		}
		return &fsutil.Journal{}, nil
	}
	if m.current(m.Root()) {
		return &fsutil.Journal{}, nil
	}
	if info, err := os.Stat(npmBinary); err != nil || info.Mode()&0o111 == 0 {
		return nil, fmt.Errorf("managed npm not found at %s; reconcile direct tools first", npmBinary)
	}
	parent := filepath.Dir(m.Root())
	if err := fsutil.GuardHome(m.Home, parent, m.Root(), filepath.Join(m.Home, ".local", "bin")); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(parent, 0o755); err != nil {
		return nil, err
	}
	workspace, err := os.MkdirTemp(parent, ".npm-")
	if err != nil {
		return nil, err
	}
	tracked := false
	defer func() {
		if !tracked {
			_ = os.RemoveAll(workspace)
		}
	}()
	stage := filepath.Join(workspace, "prefix")
	if err := os.MkdirAll(stage, 0o755); err != nil {
		return nil, err
	}
	for _, name := range []string{"package.json", "package-lock.json"} {
		if err := fsutil.CopyPath(filepath.Join(m.RepoRoot, "pde-installer", name), filepath.Join(stage, name)); err != nil {
			return nil, fmt.Errorf("stage npm %s: %w", name, err)
		}
	}
	environment := m.environment(filepath.Join(workspace, "cache"))
	if err := m.Runner.Retry("npm ci", 3, func() error {
		if err := fsutil.GuardHome(m.Home, workspace, stage); err != nil {
			return err
		}
		return m.Runner.Run("materialize pinned npm dependencies", run.Command{Name: npmBinary, Args: []string{"ci", "--ignore-scripts", "--no-audit", "--no-fund"}, Dir: stage, Env: environment})
	}); err != nil {
		return nil, err
	}
	if err := fsutil.GuardHome(m.Home, workspace, stage); err != nil {
		return nil, err
	}
	if err := m.Runner.Run("run pinned npm install scripts", run.Command{Name: npmBinary, Args: []string{"rebuild", "--foreground-scripts", "--no-audit", "--no-fund"}, Dir: stage, Env: environment}); err != nil {
		return nil, err
	}
	if err := m.verify(stage); err != nil {
		return nil, err
	}
	journal, err := fsutil.NewJournal(fsutil.JournalConfig{Home: m.Home})
	if err != nil {
		return nil, err
	}
	if err := journal.Activate(stage, m.Root()); err != nil {
		return nil, err
	}
	if err := journal.AddCleanup(workspace); err != nil {
		return nil, journal.Revert(err)
	}
	tracked = true
	for _, pkg := range packages() {
		destination := filepath.Join(m.Home, ".local", "bin", pkg.Binary)
		if err := fsutil.GuardHome(m.Home, filepath.Dir(destination)); err != nil {
			return nil, journal.Revert(err)
		}
		if err := os.MkdirAll(filepath.Dir(destination), 0o755); err != nil {
			return nil, journal.Revert(fmt.Errorf("create npm launcher directory: %w", err))
		}
		stagedLink := filepath.Join(workspace, "."+pkg.Binary+"-link")
		if err := os.Symlink(filepath.Join(m.Root(), "node_modules", ".bin", pkg.Binary), stagedLink); err != nil {
			return nil, journal.Revert(fmt.Errorf("stage npm launcher %s: %w", pkg.Binary, err))
		}
		if err := journal.Activate(stagedLink, destination); err != nil {
			return nil, journal.Revert(fmt.Errorf("activate npm launcher %s: %w", pkg.Binary, err))
		}
	}
	if err := m.verify(m.Root()); err != nil {
		return nil, journal.Revert(err)
	}
	if err := m.verifyLaunchers(); err != nil {
		return nil, journal.Revert(err)
	}
	return journal, nil
}

// ValidateLock checks the npm manifest and full dependency lock.
func (m Manager) ValidateLock() error {
	root := filepath.Join(m.RepoRoot, "pde-installer")
	data, err := os.ReadFile(filepath.Join(root, "package-lock.json"))
	if err != nil {
		return fmt.Errorf("read npm lock: %w", err)
	}
	var lock lockFile
	if err := json.Unmarshal(data, &lock); err != nil {
		return fmt.Errorf("decode npm lock: %w", err)
	}
	if lock.LockfileVersion != 3 {
		return fmt.Errorf("npm lockfile version is %d, want 3", lock.LockfileVersion)
	}
	manifestData, err := os.ReadFile(filepath.Join(root, "package.json"))
	if err != nil {
		return fmt.Errorf("read npm package manifest: %w", err)
	}
	var manifest struct {
		Dependencies map[string]string `json:"dependencies"`
	}
	if err := json.Unmarshal(manifestData, &manifest); err != nil {
		return fmt.Errorf("decode npm package manifest: %w", err)
	}
	rootEntry := lock.Packages[""]
	if len(manifest.Dependencies) != len(packages()) || len(rootEntry.Dependencies) != len(packages()) {
		return fmt.Errorf("npm manifest and lock must contain exactly %d top-level packages", len(packages()))
	}
	for path, entry := range lock.Packages {
		if path == "" || entry.Link {
			continue
		}
		if entry.Version == "" || entry.Resolved == "" || !strings.HasPrefix(entry.Integrity, "sha512-") {
			return fmt.Errorf("incomplete npm lock entry %s", path)
		}
	}
	for _, pkg := range packages() {
		entry, ok := lock.Packages["node_modules/"+pkg.Name]
		if !ok || manifest.Dependencies[pkg.Name] != pkg.Version || rootEntry.Dependencies[pkg.Name] != pkg.Version || entry.Version != pkg.Version || entry.Integrity != pkg.Integrity || !strings.HasPrefix(entry.Integrity, "sha512-") {
			return fmt.Errorf("npm lock mismatch for %s", pkg.Name)
		}
	}
	return nil
}

// Version returns an installed package's declared version.
func (m Manager) Version(name string) (string, error) { return packageVersion(m.Root(), name) }

func (m Manager) current(root string) bool {
	if m.verify(root) != nil || m.verifyLaunchers() != nil {
		return false
	}
	wanted, err := fsutil.FileSHA256(filepath.Join(m.RepoRoot, "pde-installer", "package-lock.json"))
	if err != nil {
		return false
	}
	installed, err := fsutil.FileSHA256(filepath.Join(root, "package-lock.json"))
	return err == nil && installed == wanted
}

func (m Manager) verify(root string) error {
	for _, pkg := range packages() {
		version, err := packageVersion(root, pkg.Name)
		if err != nil || version != pkg.Version {
			return fmt.Errorf("%s version is %q, want %s", pkg.Name, version, pkg.Version)
		}
		binary := filepath.Join(root, "node_modules", ".bin", pkg.Binary)
		info, err := os.Stat(binary)
		if err != nil {
			if !os.IsNotExist(err) {
				return fmt.Errorf("stat npm executable %s: %w", binary, err)
			}
			return fmt.Errorf("npm executable missing: %s", binary)
		}
		if info.Mode()&0o111 == 0 {
			return fmt.Errorf("npm executable missing: %s", binary)
		}
		output, err := m.Runner.Query("verify "+pkg.Binary, run.Command{Name: binary, Args: []string{"--version"}, Env: m.environment()})
		if err != nil {
			return fmt.Errorf("%s executable version check failed: %w", pkg.Binary, err)
		}
		if !strings.Contains(string(output), pkg.Version) {
			return fmt.Errorf("%s executable version check returned %q, want %s", pkg.Binary, strings.TrimSpace(string(output)), pkg.Version)
		}
	}
	return nil
}

func (m Manager) verifyLaunchers() error {
	for _, pkg := range packages() {
		binary := filepath.Join(m.Home, ".local", "bin", pkg.Binary)
		info, err := os.Stat(binary)
		if err != nil {
			if !os.IsNotExist(err) {
				return fmt.Errorf("stat npm launcher %s: %w", binary, err)
			}
			return fmt.Errorf("npm launcher missing: %s", binary)
		}
		if info.Mode()&0o111 == 0 {
			return fmt.Errorf("npm launcher missing: %s", binary)
		}
		output, err := m.Runner.Query("verify "+pkg.Binary+" launcher", run.Command{Name: binary, Args: []string{"--version"}, Env: m.environment()})
		if err != nil {
			return fmt.Errorf("%s launcher version check failed: %w", pkg.Binary, err)
		}
		if !strings.Contains(string(output), pkg.Version) {
			return fmt.Errorf("%s launcher version check returned %q, want %s", pkg.Binary, strings.TrimSpace(string(output)), pkg.Version)
		}
	}
	return nil
}

func packageVersion(root, name string) (string, error) {
	parts := strings.Split(name, "/")
	pathParts := append([]string{root, "node_modules"}, parts...)
	pathParts = append(pathParts, "package.json")
	data, err := os.ReadFile(filepath.Join(pathParts...))
	if err != nil {
		return "", err
	}
	var metadata struct {
		Version string `json:"version"`
	}
	if err := json.Unmarshal(data, &metadata); err != nil {
		return "", err
	}
	return metadata.Version, nil
}

func (m Manager) environment(cache ...string) []string {
	path := filepath.Join(m.Home, ".local", "bin") + string(os.PathListSeparator) + os.Getenv("PATH")
	environment := []string{"PATH=" + path}
	if len(cache) > 0 {
		environment = append(environment, "npm_config_cache="+cache[0])
	}
	return environment
}
