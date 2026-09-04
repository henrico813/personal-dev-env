package builds

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"sort"

	"pde-installer/internal/fsutil"
	"pde-installer/internal/run"
)

// Manager builds repository binaries for one installation.
type Manager struct {
	Home, RepoRoot, PkgPrefix string
	Runner                    run.Runner
}

type buildSpec struct {
	name, source, output string
	command              run.Command
}

type buildState struct {
	Inputs map[string]string `json:"inputs"`
}

type blinkState struct {
	ArchiveSHA256 string `json:"archive_sha256"`
	TreeSHA256    string `json:"tree_sha256"`
}

const (
	blinkURL    = "https://github.com/Saghen/blink.cmp/archive/2befba190e0ffa3692ab364f75604c9c2d248adf.tar.gz"
	blinkSHA256 = "25d47c66081f55bdd95d2952166aa48ad45c7bb5ebb1646678279f4d2ecc3868"
)

// New returns a local build manager.
func New(home, repoRoot, pkgPrefix string, runner run.Runner) Manager {
	return Manager{Home: home, RepoRoot: repoRoot, PkgPrefix: pkgPrefix, Runner: runner}
}

// Reconcile rebuilds binaries when their source inputs change.
func (m Manager) Reconcile() (*fsutil.Journal, error) {
	inputs, err := m.inputs()
	if err != nil {
		return nil, err
	}
	if m.current(inputs) {
		return &fsutil.Journal{}, nil
	}
	stageRoot := filepath.Join(m.Home, ".local", "state", "pde", "build-stage")
	if m.Runner.DryRun {
		for _, name := range []string{"planner", "opencode-inline-shim", "surveil", "vibe"} {
			if err := m.Runner.Plan("build and atomically activate "+name, nil); err != nil {
				return nil, err
			}
		}
		return &fsutil.Journal{}, nil
	}
	if err := fsutil.GuardHome(m.Home, stageRoot, m.statePath(), filepath.Join(m.Home, ".local", "bin")); err != nil {
		return nil, err
	}
	if err := os.RemoveAll(stageRoot); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(stageRoot, 0o755); err != nil {
		return nil, err
	}
	tracked := false
	defer func() {
		if !tracked {
			_ = os.RemoveAll(stageRoot)
		}
	}()
	goBinary := filepath.Join(m.PkgPrefix, "bin", "go")
	cargo := filepath.Join(m.PkgPrefix, "bin", "cargo")
	environment := m.environment(stageRoot)
	specs := []buildSpec{
		{name: "planner", source: filepath.Join(m.RepoRoot, "planner"), output: filepath.Join(stageRoot, "planner"), command: run.Command{Name: goBinary, Args: []string{"build", "-mod=readonly", "-o", filepath.Join(stageRoot, "planner"), "./main"}, Dir: filepath.Join(m.RepoRoot, "planner"), Env: environment}},
		{name: "opencode-inline-shim", source: filepath.Join(m.RepoRoot, "cli"), output: filepath.Join(stageRoot, "opencode-inline-shim"), command: run.Command{Name: goBinary, Args: []string{"build", "-mod=readonly", "-o", filepath.Join(stageRoot, "opencode-inline-shim"), "./cmd/opencode-inline-shim"}, Dir: filepath.Join(m.RepoRoot, "cli"), Env: environment}},
		{name: "surveil", source: filepath.Join(m.RepoRoot, "surveil"), output: filepath.Join(stageRoot, "surveil-target", "release", "surveil"), command: run.Command{Name: cargo, Args: []string{"build", "--locked", "--release", "--target-dir", filepath.Join(stageRoot, "surveil-target"), "--bin", "surveil"}, Dir: filepath.Join(m.RepoRoot, "surveil"), Env: environment}},
		{name: "vibe", source: filepath.Join(m.RepoRoot, "vibe"), output: filepath.Join(stageRoot, "vibe-target", "release", "vibe"), command: run.Command{Name: cargo, Args: []string{"build", "--locked", "--release", "--target-dir", filepath.Join(stageRoot, "vibe-target")}, Dir: filepath.Join(m.RepoRoot, "vibe"), Env: environment}},
	}
	for _, spec := range specs {
		if err := fsutil.GuardHome(m.Home, stageRoot); err != nil {
			return nil, err
		}
		if err := m.Runner.Run("build "+spec.name, spec.command); err != nil {
			return nil, err
		}
	}
	journal, err := fsutil.NewJournal(fsutil.JournalConfig{Home: m.Home})
	if err != nil {
		return nil, err
	}
	for _, spec := range specs {
		staged := filepath.Join(stageRoot, "activate-"+spec.name)
		if err := copyExecutable(spec.output, staged); err != nil {
			return nil, journal.Revert(fmt.Errorf("stage %s: %w", spec.name, err))
		}
		if err := journal.Activate(staged, filepath.Join(m.Home, ".local", "bin", spec.name)); err != nil {
			return nil, journal.Revert(fmt.Errorf("activate %s: %w", spec.name, err))
		}
		if !tracked {
			if err := journal.AddCleanup(stageRoot); err != nil {
				return nil, journal.Revert(err)
			}
			tracked = true
		}
	}
	for _, check := range []struct{ name, arg string }{{"planner", "help"}, {"opencode-inline-shim", "--help"}, {"surveil", "--help"}, {"vibe", "--help"}} {
		if err := m.Runner.Run("verify "+check.name, run.Command{Name: filepath.Join(m.Home, ".local", "bin", check.name), Args: []string{check.arg}, Env: environment}); err != nil {
			return nil, journal.Revert(err)
		}
	}
	stateData, err := json.Marshal(buildState{Inputs: inputs})
	if err != nil {
		return nil, journal.Revert(fmt.Errorf("encode build state: %w", err))
	}
	stateStage := filepath.Join(stageRoot, "builds.json")
	if err := os.WriteFile(stateStage, append(stateData, '\n'), 0o644); err != nil {
		return nil, journal.Revert(fmt.Errorf("write build state: %w", err))
	}
	if err := journal.Activate(stateStage, m.statePath()); err != nil {
		return nil, journal.Revert(fmt.Errorf("activate build state: %w", err))
	}
	return journal, nil
}

// BuildBlink builds and activates the pinned blink.cmp native library.
func (m Manager) BuildBlink() (*fsutil.Journal, error) {
	destination := filepath.Join(m.Home, ".config", "nvim", "pack", "plugins", "start", "blink.cmp")
	blinkLib := filepath.Join(m.Home, ".config", "nvim", "pack", "plugins", "start", "blink.lib")
	if m.Runner.DryRun {
		if err := m.Runner.Plan("download and verify "+blinkURL+" sha256="+blinkSHA256, nil); err != nil {
			return nil, err
		}
		if err := m.Runner.Plan("build, verify, and atomically activate complete blink.cmp plugin tree", nil); err != nil {
			return nil, err
		}
		return &fsutil.Journal{}, nil
	}
	statePath := filepath.Join(m.Home, ".local", "state", "pde", "blink.json")
	if data, err := os.ReadFile(statePath); err == nil {
		var state blinkState
		installedHash, hashErr := hashTree(destination)
		if json.Unmarshal(data, &state) == nil && hashErr == nil && state.ArchiveSHA256 == blinkSHA256 && state.TreeSHA256 == installedHash && regularExecutable(filepath.Join(destination, "lib", "libblink_cmp_fuzzy.so")) {
			return &fsutil.Journal{}, nil
		}
	}
	stateDir := filepath.Dir(statePath)
	if err := fsutil.GuardHome(m.Home, stateDir, destination); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(stateDir, 0o755); err != nil {
		return nil, err
	}
	workspace, err := os.MkdirTemp(stateDir, ".blink-")
	if err != nil {
		return nil, err
	}
	tracked := false
	defer func() {
		if !tracked {
			_ = os.RemoveAll(workspace)
		}
	}()
	archive, stage := filepath.Join(workspace, "blink.tar.gz"), filepath.Join(workspace, "plugin")
	if err := fsutil.GuardHome(m.Home, workspace, archive, stage); err != nil {
		return nil, err
	}
	if err := m.Runner.Retry("blink.cmp download", 3, func() error {
		if err := fsutil.GuardHome(m.Home, workspace, archive); err != nil {
			return err
		}
		return fsutil.Download(blinkURL, archive, blinkSHA256)
	}); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(stage, 0o755); err != nil {
		return nil, err
	}
	if err := m.Runner.Run("extract blink.cmp", run.Command{Name: "tar", Args: []string{"-xzf", archive, "--strip-components=1", "-C", stage}}); err != nil {
		return nil, err
	}
	target := filepath.Join(workspace, "target")
	command := run.Command{Name: filepath.Join(m.PkgPrefix, "bin", "cargo"), Args: []string{"build", "--locked", "--release", "--target-dir", target}, Dir: stage, Env: m.environment(workspace)}
	if err := fsutil.GuardHome(m.Home, workspace, stage, target); err != nil {
		return nil, err
	}
	if err := m.Runner.Run("build blink.cmp", command); err != nil {
		return nil, err
	}
	if err := copyExecutable(filepath.Join(target, "release", "libblink_cmp_fuzzy.so"), filepath.Join(stage, "lib", "libblink_cmp_fuzzy.so")); err != nil {
		return nil, err
	}
	nvim := filepath.Join(m.PkgPrefix, "bin", "nvim")
	lua := "assert(require('blink.cmp').library_available(), 'blink native library unavailable')"
	verify := run.Command{Name: nvim, Args: []string{"--headless", "-u", "NONE", "--cmd", "set runtimepath+=" + blinkLib, "--cmd", "set runtimepath+=" + stage, "-c", "lua " + lua, "-c", "qa"}, Env: m.environment(workspace)}
	if err := m.Runner.Run("verify blink.cmp native library", verify); err != nil {
		return nil, err
	}
	treeHash, err := hashTree(stage)
	if err != nil {
		return nil, err
	}
	stateData, err := json.Marshal(blinkState{ArchiveSHA256: blinkSHA256, TreeSHA256: treeHash})
	if err != nil {
		return nil, err
	}
	stateStage := filepath.Join(workspace, "blink.json")
	if err := os.WriteFile(stateStage, append(stateData, '\n'), 0o644); err != nil {
		return nil, fmt.Errorf("write blink state: %w", err)
	}
	journal, err := fsutil.NewJournal(fsutil.JournalConfig{Home: m.Home})
	if err != nil {
		return nil, err
	}
	if err := journal.Activate(stage, destination); err != nil {
		return nil, err
	}
	if err := journal.AddCleanup(workspace); err != nil {
		return nil, journal.Revert(err)
	}
	tracked = true
	if err := journal.Activate(stateStage, statePath); err != nil {
		return nil, journal.Revert(fmt.Errorf("activate blink state: %w", err))
	}
	return journal, nil
}

// Status reports whether a built binary is installed.
func (m Manager) Status(name string) string {
	status, err := m.Probe(name)
	if err != nil {
		return "error"
	}
	return status
}

// Probe reports build state without hiding filesystem errors.
func (m Manager) Probe(name string) (string, error) {
	path := filepath.Join(m.Home, ".local", "bin", name)
	info, err := os.Stat(path)
	if err != nil {
		if os.IsNotExist(err) {
			return "missing", nil
		}
		return "", fmt.Errorf("stat built binary %s: %w", name, err)
	}
	if info.Mode().IsRegular() && info.Mode()&0o111 != 0 && info.Size() > 0 {
		return "installed", nil
	}
	return "missing", nil
}

func (m Manager) inputs() (map[string]string, error) {
	inputs := map[string]string{}
	for name, source := range map[string]string{"planner": filepath.Join(m.RepoRoot, "planner"), "opencode-inline-shim": filepath.Join(m.RepoRoot, "cli"), "surveil": filepath.Join(m.RepoRoot, "surveil"), "vibe": filepath.Join(m.RepoRoot, "vibe")} {
		hash, err := hashTree(source)
		if err != nil {
			return nil, err
		}
		inputs[name] = hash
	}
	return inputs, nil
}

func (m Manager) current(inputs map[string]string) bool {
	data, err := os.ReadFile(m.statePath())
	if err != nil {
		return false
	}
	var state buildState
	if json.Unmarshal(data, &state) != nil || len(state.Inputs) != len(inputs) {
		return false
	}
	for name, input := range inputs {
		if state.Inputs[name] != input || !regularExecutable(filepath.Join(m.Home, ".local", "bin", name)) {
			return false
		}
	}
	return true
}

func (m Manager) statePath() string {
	return filepath.Join(m.Home, ".local", "state", "pde", "builds.json")
}

func (m Manager) environment(stage ...string) []string {
	path := filepath.Join(m.PkgPrefix, "bin") + string(os.PathListSeparator) + filepath.Join(m.PkgPrefix, "sbin") + string(os.PathListSeparator) + os.Getenv("PATH")
	environment := []string{"PATH=" + path, "PKG_CONFIG_PATH=" + filepath.Join(m.PkgPrefix, "lib", "pkgconfig")}
	if len(stage) > 0 {
		environment = append(environment, "GOCACHE="+filepath.Join(stage[0], "go-cache"), "GOMODCACHE="+filepath.Join(stage[0], "go-mod-cache"), "CARGO_HOME="+filepath.Join(stage[0], "cargo-home"))
	}
	return environment
}

func hashTree(root string) (string, error) {
	var paths []string
	err := filepath.WalkDir(root, func(path string, entry fs.DirEntry, walkErr error) error {
		if walkErr != nil {
			return walkErr
		}
		if entry.IsDir() && path != root {
			if entry.Name() == ".git" || entry.Name() == "target" {
				return filepath.SkipDir
			}
		}
		if entry.Type().IsRegular() {
			paths = append(paths, path)
		}
		return nil
	})
	if err != nil {
		return "", err
	}
	sort.Strings(paths)
	hash := sha256.New()
	for _, path := range paths {
		relative, _ := filepath.Rel(root, path)
		_, _ = io.WriteString(hash, relative+"\x00")
		file, err := os.Open(path)
		if err != nil {
			return "", err
		}
		_, copyErr := io.Copy(hash, file)
		closeErr := file.Close()
		if copyErr != nil {
			return "", copyErr
		}
		if closeErr != nil {
			return "", closeErr
		}
	}
	return hex.EncodeToString(hash.Sum(nil)), nil
}

func regularExecutable(path string) bool {
	info, err := os.Stat(path)
	return err == nil && info.Mode().IsRegular() && info.Mode()&0o111 != 0 && info.Size() > 0
}

func copyExecutable(source, destination string) error {
	input, err := os.Open(source)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(filepath.Dir(destination), 0o755); err != nil {
		_ = input.Close()
		return err
	}
	output, err := os.OpenFile(destination, os.O_CREATE|os.O_TRUNC|os.O_WRONLY, 0o755)
	if err != nil {
		_ = input.Close()
		return err
	}
	_, copyErr := output.ReadFrom(input)
	inputErr, outputErr := input.Close(), output.Close()
	if copyErr != nil {
		return copyErr
	}
	if inputErr != nil {
		return inputErr
	}
	return outputErr
}
