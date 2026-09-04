package aqua

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"runtime"
	"strings"

	"pde-installer/internal/fsutil"
	"pde-installer/internal/run"
)

const version = "v2.60.1"

type tool struct {
	name    string
	version string
}

func checksum(architecture string) (string, bool) {
	switch architecture {
	case "amd64":
		return "d6f920201c71fb42881af51f8f63c3f06da778b38399248b2c777a288ebe3884", true
	case "arm64":
		return "c203124300502abc7f338a0d460810cee769049d44348c8fadb5eee90119ecdd", true
	default:
		return "", false
	}
}

func tools() []tool {
	return []tool{
		{name: "fd", version: "v8.3.1"}, {name: "fzf", version: "0.36.0"},
		{name: "ripgrep", version: "14.1.1"}, {name: "bat", version: "v0.19.0"},
		{name: "jq", version: "jq-1.7.1"}, {name: "chezmoi", version: "v2.72.0"},
		{name: "eza", version: "v0.23.4"}, {name: "zoxide", version: "v0.9.8"},
		{name: "bottom", version: "0.11.4"}, {name: "yq", version: "v4.53.3"},
		{name: "yazi", version: "v25.5.31"}, {name: "ya", version: "v25.5.31"},
	}
}

// Manager installs Aqua and its pinned tool set for one user.
type Manager struct {
	home, repoRoot, root string
	runner               run.Runner
}

type state struct {
	ConfigSHA256    string `json:"config_sha256"`
	ChecksumsSHA256 string `json:"checksums_sha256"`
}

// New returns an Aqua manager rooted in the user's home directory.
func New(home, repoRoot string, runner run.Runner) Manager {
	return Manager{home: home, repoRoot: repoRoot, root: filepath.Join(home, ".local", "share", "aquaproj-aqua"), runner: runner}
}

func (m Manager) binary() string { return filepath.Join(m.root, "bin", "aqua") }

// Reconcile installs the pinned Aqua release and managed tools.
func (m Manager) Reconcile() (*fsutil.Journal, error) {
	checksum, ok := checksum(runtime.GOARCH)
	if runtime.GOOS != "linux" || !ok {
		return nil, fmt.Errorf("unsupported Aqua platform %s/%s", runtime.GOOS, runtime.GOARCH)
	}
	config := filepath.Join(m.repoRoot, "chezmoi", "dot_config", "aquaproj-aqua", "aqua.yaml")
	checksumsFile := filepath.Join(filepath.Dir(config), "aqua-checksums.json")
	for _, path := range []string{config, checksumsFile} {
		if info, err := os.Stat(path); err != nil || !info.Mode().IsRegular() {
			return nil, fmt.Errorf("required Aqua source is not a regular file: %s", path)
		}
	}
	wanted, err := pinState(config, checksumsFile)
	if err != nil {
		return nil, err
	}
	url := fmt.Sprintf("https://github.com/aquaproj/aqua/releases/download/%s/aqua_linux_%s.tar.gz", version, runtime.GOARCH)
	if m.runner.DryRun {
		if err := m.runner.Plan("download and verify "+url+" sha256="+checksum, nil); err != nil {
			return nil, err
		}
		if err := m.runner.Plan("install Aqua packages in staging root", nil); err != nil {
			return nil, err
		}
		if err := m.runner.Plan("atomically activate "+m.root, nil); err != nil {
			return nil, err
		}
		return &fsutil.Journal{}, nil
	}
	parent := filepath.Dir(m.root)
	if err := fsutil.GuardHome(m.home, parent, m.root); err != nil {
		return nil, err
	}
	if m.current(wanted) {
		return &fsutil.Journal{}, nil
	}
	if err := os.MkdirAll(parent, 0o755); err != nil {
		return nil, err
	}
	stage, err := os.MkdirTemp(parent, ".aqua-")
	if err != nil {
		return nil, err
	}
	activated := false
	defer func() {
		if !activated {
			_ = os.RemoveAll(stage)
		}
	}()
	archive := filepath.Join(stage, "aqua.tar.gz")
	if err := m.runner.Retry("Aqua download", 3, func() error {
		if err := fsutil.GuardHome(m.home, stage, archive); err != nil {
			return err
		}
		return fsutil.Download(url, archive, checksum)
	}); err != nil {
		return nil, err
	}
	if err := m.runner.Run("extract Aqua", run.Command{Name: "tar", Args: []string{"-xzf", archive, "-C", stage}}); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(filepath.Join(stage, "bin"), 0o755); err != nil {
		return nil, err
	}
	binary := filepath.Join(stage, "bin", "aqua")
	if err := os.Rename(filepath.Join(stage, "aqua"), binary); err != nil {
		return nil, err
	}
	if err := os.Chmod(binary, 0o755); err != nil {
		return nil, err
	}
	stageConfig := filepath.Join(stage, "config")
	if err := os.MkdirAll(stageConfig, 0o755); err != nil {
		return nil, err
	}
	stagedManifest, stagedChecksums := filepath.Join(stageConfig, "aqua.yaml"), filepath.Join(stageConfig, "aqua-checksums.json")
	if err := fsutil.CopyPath(config, stagedManifest); err != nil {
		return nil, err
	}
	if err := fsutil.CopyPath(checksumsFile, stagedChecksums); err != nil {
		return nil, err
	}
	environment := []string{"AQUA_ROOT_DIR=" + stage, "AQUA_GLOBAL_CONFIG=" + stagedManifest, "AQUA_CHECKSUMS_PATH=" + stagedChecksums, "PATH=" + filepath.Join(stage, "bin") + string(os.PathListSeparator) + os.Getenv("PATH")}
	if err := fsutil.GuardHome(m.home, stage); err != nil {
		return nil, err
	}
	if err := m.runner.Run("reconcile Aqua packages", run.Command{Name: binary, Args: []string{"--config", stagedManifest, "install"}, Env: environment}); err != nil {
		return nil, err
	}
	_ = os.Remove(archive)
	_ = os.RemoveAll(stageConfig)
	stateData, err := json.Marshal(wanted)
	if err != nil {
		return nil, err
	}
	if err := os.WriteFile(filepath.Join(stage, ".pde-state.json"), append(stateData, '\n'), 0o644); err != nil {
		return nil, err
	}
	journal := &fsutil.Journal{Home: m.home}
	if err := journal.Activate(stage, m.root); err != nil {
		return nil, err
	}
	activated = true
	return journal, nil
}

func (m Manager) current(wanted state) bool {
	data, err := os.ReadFile(filepath.Join(m.root, ".pde-state.json"))
	if err != nil {
		return false
	}
	var installed state
	if json.Unmarshal(data, &installed) != nil || installed != wanted {
		return false
	}
	if _, state := m.Status(); state != "current" {
		return false
	}
	for _, candidate := range tools() {
		if _, state := m.ToolStatus(candidate.name, candidate.version); state != "current" {
			return false
		}
	}
	return true
}

func pinState(config, checksums string) (state, error) {
	configHash, err := fsutil.FileSHA256(config)
	if err != nil {
		return state{}, fmt.Errorf("hash Aqua config: %w", err)
	}
	checksumsHash, err := fsutil.FileSHA256(checksums)
	if err != nil {
		return state{}, fmt.Errorf("hash Aqua checksums: %w", err)
	}
	return state{ConfigSHA256: configHash, ChecksumsSHA256: checksumsHash}, nil
}

// Status reports the installed Aqua version and state.
func (m Manager) Status() (string, string) {
	output, err := m.runner.Query("read Aqua version", run.Command{Name: m.binary(), Args: []string{"--version"}})
	if err != nil {
		return "", "missing"
	}
	installed := strings.TrimSpace(string(output))
	if containsVersion(installed, strings.TrimPrefix(version, "v")) {
		return installed, "current"
	}
	return installed, "outdated"
}

// ToolStatus reports the installed version and state of a managed tool.
func (m Manager) ToolStatus(name, version string) (string, string) {
	binaryNames := map[string]string{"ripgrep": "rg", "bottom": "btm"}
	binary := name
	if mapped := binaryNames[name]; mapped != "" {
		binary = mapped
	}
	path := m.installedBinary(binary, version)
	if path == "" {
		return "", "missing"
	}
	output, err := m.runner.Query("read "+name+" version", run.Command{Name: path, Args: []string{"--version"}})
	if err != nil {
		return "", "missing"
	}
	lines := strings.Split(strings.TrimSpace(string(output)), "\n")
	installed := strings.TrimSpace(lines[0])
	wanted := strings.TrimPrefix(strings.TrimPrefix(version, "v"), "jq-")
	if containsVersion(string(output), wanted) {
		for _, line := range lines {
			if containsVersion(line, wanted) {
				installed = strings.TrimSpace(line)
				break
			}
		}
		return installed, "current"
	}
	return installed, "outdated"
}

func (m Manager) installedBinary(name, version string) string {
	root := filepath.Join(m.root, "pkgs")
	wanted := string(filepath.Separator) + version + string(filepath.Separator)
	var matches []string
	_ = filepath.WalkDir(root, func(path string, entry os.DirEntry, err error) error {
		if err != nil {
			return err
		}
		if entry.Type().IsRegular() && entry.Name() == name && strings.Contains(path, wanted) {
			info, infoErr := entry.Info()
			if infoErr == nil && info.Mode()&0o111 != 0 {
				matches = append(matches, path)
			}
		}
		return nil
	})
	if len(matches) != 1 {
		return ""
	}
	return matches[0]
}

func containsVersion(output, version string) bool {
	pattern := `(^|[^0-9])v?` + regexp.QuoteMeta(version) + `($|[^0-9])`
	return regexp.MustCompile(pattern).MatchString(output)
}
