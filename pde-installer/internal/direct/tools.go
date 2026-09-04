package direct

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strings"

	"pde-installer/internal/fsutil"
	"pde-installer/internal/manifest"
	"pde-installer/internal/run"
)

type toolKind uint8

const (
	archiveTool toolKind = iota + 1
	rustTool
	fileTool
)

// Tool identifies one platform-specific release archive.
type Tool struct {
	Name, Version, Archive, URL, SHA256 string
	Directory, Binary, VersionPrefix    string
	Target                              string
	Links                               []string
	VersionArgs                         []string
	Kind                                toolKind
}

type toolState struct {
	Version string `json:"version"`
	SHA256  string `json:"sha256"`
}

type toolsState struct {
	Platform string               `json:"platform"`
	Tools    map[string]toolState `json:"tools"`
}

// Tools returns direct-release tools for the current platform.
func Tools() ([]Tool, error) {
	return toolsForPlatform(runtime.GOOS, runtime.GOARCH)
}

func toolsForPlatform(goos, goarch string) ([]Tool, error) {
	if goos != "linux" {
		return nil, fmt.Errorf("direct tools do not support %s/%s", goos, goarch)
	}

	var nvimArch, rustArch, nodeArch string
	var checksums map[string]string
	switch goarch {
	case "amd64":
		nvimArch, rustArch, nodeArch = "x86_64", "x86_64", "x64"
		checksums = map[string]string{
			"neovim": "c441b547142860bf01bcce39e36cbed185c41112813e15443b16e5237750724d",
			"go":     "1153d3d50e0ac764b447adfe05c2bcf08e889d42a02e0fe0259bd47f6733ad7f",
			"rust":   "c295047583a56238ea06b43f849f4b877fa12bfd4c7103f8d9a74c94c9c4e108",
			"node":   "d804845d34eddc21dc1092b519d643ef40b1f58ec5dec5c22b1f4bd8fabde6c9",
		}
	case "arm64":
		nvimArch, rustArch, nodeArch = "arm64", "aarch64", "arm64"
		checksums = map[string]string{
			"neovim": "e055af73fa9c72b37456da8d204fa5c09850bc07e80e9176fe3b87d4afb7a3fc",
			"go":     "ef758ae7c6cf9267c9c0ef080b8965f453d89ab2d25d9eb22de4405925238768",
			"rust":   "371eadcca97062219cbd8593628eb5d2802bc370515d085fedce1b56b2baed57",
			"node":   "524659219d6a207a7400f2bde15d19ba060ffbe0d32a8643319ad67e3bb64c78",
		}
	default:
		return nil, fmt.Errorf("direct tools do not support %s/%s", goos, goarch)
	}

	version := func(name string) string {
		item, _ := manifest.Find(name, manifest.Direct)
		return item.Version
	}
	nvimVersion, goVersion := version("neovim"), version("go")
	rustVersion, nodeVersion := version("rust"), version("node")
	nvimArchive := "nvim-linux-" + nvimArch + ".tar.gz"
	goArchive := "go" + goVersion + ".linux-" + goarch + ".tar.gz"
	rustArchive := "rust-" + rustVersion + "-" + rustArch + "-unknown-linux-gnu.tar.xz"
	nodeArchive := "node-v" + nodeVersion + "-linux-" + nodeArch + ".tar.xz"
	return []Tool{
		{Name: "neovim", Version: nvimVersion, Archive: nvimArchive, URL: "https://github.com/neovim/neovim/releases/download/v" + nvimVersion + "/" + nvimArchive, SHA256: checksums["neovim"], Directory: "neovim", Binary: "nvim", VersionPrefix: "NVIM v", Links: []string{"nvim"}, VersionArgs: []string{"--version"}, Kind: archiveTool},
		{Name: "go", Version: goVersion, Archive: goArchive, URL: "https://go.dev/dl/" + goArchive, SHA256: checksums["go"], Directory: "go", Binary: "go", VersionPrefix: "go version go", Links: []string{"go", "gofmt"}, VersionArgs: []string{"version"}, Kind: archiveTool},
		{Name: "rust", Version: rustVersion, Archive: rustArchive, URL: "https://static.rust-lang.org/dist/" + rustArchive, SHA256: checksums["rust"], Directory: "rust", Binary: "rustc", VersionPrefix: "rustc ", Target: rustArch + "-unknown-linux-gnu", Links: []string{"cargo", "rustc", "rustdoc"}, VersionArgs: []string{"--version"}, Kind: rustTool},
		{Name: "node", Version: nodeVersion, Archive: nodeArchive, URL: "https://nodejs.org/dist/v" + nodeVersion + "/" + nodeArchive, SHA256: checksums["node"], Directory: "node", Binary: "node", VersionPrefix: "v", Links: []string{"corepack", "node", "npm", "npx"}, VersionArgs: []string{"--version"}, Kind: archiveTool},
		{Name: "keychain", Version: version("keychain"), Archive: "keychain-2.9.8", URL: "https://github.com/danielrobbins/keychain/releases/download/2.9.8/keychain", SHA256: "f8b4e8a2a630907bb81737d455a2dec2cb8308e3210840665239ef9c49bbeadb", Directory: "keychain", Binary: "keychain", VersionPrefix: "keychain ", Links: []string{"keychain"}, VersionArgs: []string{"--version"}, Kind: fileTool},
	}, nil
}

// ReconcileTools installs and activates all pinned release tools.
func (m Manager) ReconcileTools() (*fsutil.Journal, error) {
	tools, err := Tools()
	if err != nil {
		return nil, err
	}
	return m.reconcileTools(tools)
}

func (m Manager) reconcileTools(tools []Tool) (*fsutil.Journal, error) {
	current, err := m.toolsCurrent(tools)
	if err != nil {
		return nil, err
	}
	if current {
		return &fsutil.Journal{}, nil
	}
	if m.Runner.DryRun {
		for _, tool := range tools {
			if err := m.Runner.Plan("download and verify "+tool.URL+" sha256="+tool.SHA256, nil); err != nil {
				return nil, err
			}
		}
		if err := m.Runner.Plan("atomically activate direct release tools", nil); err != nil {
			return nil, err
		}
		return &fsutil.Journal{}, nil
	}

	root := m.ToolsRoot()
	parent := filepath.Dir(root)
	localBin := filepath.Join(m.Home, ".local", "bin")
	if err := fsutil.GuardHome(m.Home, parent, root, localBin); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(parent, 0o755); err != nil {
		return nil, fmt.Errorf("create direct tool parent: %w", err)
	}
	workspace, err := os.MkdirTemp(parent, ".tools-")
	if err != nil {
		return nil, fmt.Errorf("create direct tool workspace: %w", err)
	}
	journal, err := fsutil.NewJournal(fsutil.JournalConfig{Home: m.Home})
	if err != nil {
		_ = os.RemoveAll(workspace)
		return nil, err
	}
	if err := journal.AddCleanup(workspace); err != nil {
		_ = os.RemoveAll(workspace)
		return nil, err
	}
	tracked := false
	defer func() {
		if !tracked {
			_ = journal.Rollback()
		}
	}()
	stage := filepath.Join(workspace, "releases")
	if err := os.MkdirAll(filepath.Join(stage, "bin"), 0o755); err != nil {
		return nil, err
	}
	for _, tool := range tools {
		if err := m.installTool(workspace, stage, tool); err != nil {
			return nil, fmt.Errorf("install %s: %w", tool.Name, err)
		}
	}
	if err := m.writeToolsState(stage, tools); err != nil {
		return nil, err
	}
	if err := m.verifyTools(stage, tools); err != nil {
		return nil, err
	}

	if err := journal.Activate(stage, root); err != nil {
		return nil, fmt.Errorf("activate direct tools: %w", err)
	}
	tracked = true
	if err := os.MkdirAll(localBin, 0o755); err != nil {
		return nil, journal.Revert(fmt.Errorf("create local bin: %w", err))
	}
	for _, tool := range tools {
		for _, name := range tool.Links {
			stagedLink := filepath.Join(workspace, "."+name+"-link")
			if err := os.Symlink(filepath.Join(root, "bin", name), stagedLink); err != nil {
				return nil, journal.Revert(fmt.Errorf("stage %s launcher: %w", name, err))
			}
			if err := journal.Activate(stagedLink, filepath.Join(localBin, name)); err != nil {
				return nil, journal.Revert(fmt.Errorf("activate %s launcher: %w", name, err))
			}
		}
	}
	if err := m.verifyTools(root, tools); err != nil {
		return nil, journal.Revert(err)
	}
	return journal, nil
}

func (m Manager) installTool(workspace, stage string, tool Tool) error {
	archive := filepath.Join(workspace, tool.Archive)
	if err := fsutil.GuardHome(m.Home, workspace, archive, stage); err != nil {
		return err
	}
	if err := m.Runner.Retry("direct tool download "+tool.Name, 3, func() error {
		return fsutil.Download(tool.URL, archive, tool.SHA256)
	}); err != nil {
		return err
	}
	destination := filepath.Join(stage, tool.Directory)
	if err := os.MkdirAll(destination, 0o755); err != nil {
		return err
	}
	switch tool.Kind {
	case rustTool:
		extracted := filepath.Join(workspace, "rust-installer")
		if err := os.MkdirAll(extracted, 0o755); err != nil {
			return err
		}
		if err := m.extractTool(tool, archive, extracted); err != nil {
			return err
		}
		components := strings.Join([]string{"rustc", "rust-std-" + tool.Target, "cargo"}, ",")
		args := []string{filepath.Join(extracted, "install.sh"), "--prefix=" + destination, "--disable-ldconfig", "--components=" + components}
		if err := m.Runner.Run("install rust release", run.Command{Name: "sh", Args: args}); err != nil {
			return err
		}
	case archiveTool:
		if err := m.extractTool(tool, archive, destination); err != nil {
			return err
		}
	case fileTool:
		bin := filepath.Join(destination, "bin")
		if err := os.MkdirAll(bin, 0o755); err != nil {
			return err
		}
		target := filepath.Join(bin, tool.Binary)
		if err := os.Rename(archive, target); err != nil {
			return fmt.Errorf("stage %s executable: %w", tool.Name, err)
		}
		if err := os.Chmod(target, 0o755); err != nil {
			return fmt.Errorf("make %s executable: %w", tool.Name, err)
		}
	default:
		return fmt.Errorf("unsupported direct tool kind %d", tool.Kind)
	}
	for _, name := range tool.Links {
		target := filepath.Join("..", tool.Directory, "bin", name)
		if err := os.Symlink(target, filepath.Join(stage, "bin", name)); err != nil {
			return fmt.Errorf("link %s: %w", name, err)
		}
	}
	return nil
}

func (m Manager) extractTool(tool Tool, archive, destination string) error {
	flag := "-xJf"
	if strings.HasSuffix(tool.Archive, ".tar.gz") {
		flag = "-xzf"
	}
	return m.Runner.Run("extract "+tool.Name, run.Command{Name: "tar", Args: []string{flag, archive, "--strip-components=1", "-C", destination}})
}

func (m Manager) writeToolsState(stage string, tools []Tool) error {
	state := toolsState{Platform: runtime.GOOS + "/" + runtime.GOARCH, Tools: map[string]toolState{}}
	for _, tool := range tools {
		state.Tools[tool.Name] = toolState{Version: tool.Version, SHA256: tool.SHA256}
	}
	data, err := json.Marshal(state)
	if err != nil {
		return fmt.Errorf("encode direct tool state: %w", err)
	}
	if err := os.WriteFile(filepath.Join(stage, ".pde-state.json"), append(data, '\n'), 0o644); err != nil {
		return fmt.Errorf("write direct tool state: %w", err)
	}
	return nil
}

func (m Manager) toolsCurrent(tools []Tool) (bool, error) {
	data, err := os.ReadFile(filepath.Join(m.ToolsRoot(), ".pde-state.json"))
	if os.IsNotExist(err) {
		return false, nil
	}
	if err != nil {
		return false, fmt.Errorf("read direct tool state: %w", err)
	}
	var state toolsState
	if json.Unmarshal(data, &state) != nil || state.Platform != runtime.GOOS+"/"+runtime.GOARCH || len(state.Tools) != len(tools) {
		return false, nil
	}
	for _, tool := range tools {
		installed := state.Tools[tool.Name]
		if installed.Version != tool.Version || installed.SHA256 != tool.SHA256 {
			return false, nil
		}
		launchersCurrent, err := m.toolLaunchersCurrent(tool)
		if err != nil {
			return false, err
		}
		if !launchersCurrent {
			return false, nil
		}
	}
	return m.verifyTools(m.ToolsRoot(), tools) == nil, nil
}

func (m Manager) verifyTools(root string, tools []Tool) error {
	environment := []string{"PATH=" + filepath.Join(root, "bin") + string(os.PathListSeparator) + os.Getenv("PATH")}
	for _, tool := range tools {
		for _, name := range tool.Links {
			path := filepath.Join(root, "bin", name)
			info, err := os.Stat(path)
			if err != nil {
				return fmt.Errorf("stat %s: %w", name, err)
			}
			if !info.Mode().IsRegular() || info.Mode()&0o111 == 0 {
				return fmt.Errorf("direct tool executable is invalid: %s", path)
			}
		}
		if tool.Kind == fileTool {
			digest, err := fsutil.FileSHA256(filepath.Join(root, tool.Directory, "bin", tool.Binary))
			if err != nil {
				return fmt.Errorf("hash %s: %w", tool.Name, err)
			}
			if digest != tool.SHA256 {
				return fmt.Errorf("%s executable checksum is %s, want %s", tool.Name, digest, tool.SHA256)
			}
			continue
		}
		output, err := m.Runner.Query("verify "+tool.Name, run.Command{Name: filepath.Join(root, "bin", tool.Binary), Args: tool.VersionArgs, Env: environment})
		if err != nil {
			return err
		}
		if !hasToolVersion(string(output), tool) {
			return fmt.Errorf("%s version check returned %q, want %s", tool.Name, strings.TrimSpace(string(output)), tool.Version)
		}
	}
	return nil
}

// ToolsRoot returns the managed direct-release prefix.
func (m Manager) ToolsRoot() string {
	return filepath.Join(m.Home, ".local", "share", "pde", "releases")
}

// ToolProbe reports the installed version and state of a direct tool.
func (m Manager) ToolProbe(name string) (string, string, error) {
	tools, err := Tools()
	if err != nil {
		return "", "", err
	}
	for _, tool := range tools {
		if tool.Name != name {
			continue
		}
		launchersCurrent, err := m.toolLaunchersCurrent(tool)
		if err != nil {
			return "", "", err
		}
		if !launchersCurrent {
			return "", "missing", nil
		}
		binary := filepath.Join(m.ToolsRoot(), "bin", tool.Binary)
		if _, err := os.Stat(binary); os.IsNotExist(err) {
			return "", "missing", nil
		} else if err != nil {
			return "", "", fmt.Errorf("stat %s: %w", name, err)
		}
		if tool.Kind == fileTool {
			digest, err := fsutil.FileSHA256(filepath.Join(m.ToolsRoot(), tool.Directory, "bin", tool.Binary))
			if err != nil {
				return "", "", fmt.Errorf("hash %s: %w", name, err)
			}
			if digest == tool.SHA256 {
				return tool.Version, "current", nil
			}
			return "", "outdated", nil
		}
		output, err := m.Runner.Query("read "+name+" version", run.Command{Name: binary, Args: tool.VersionArgs, Env: []string{"PATH=" + filepath.Join(m.ToolsRoot(), "bin") + string(os.PathListSeparator) + os.Getenv("PATH")}})
		if err != nil {
			return "", "broken", err
		}
		installed := strings.TrimSpace(strings.SplitN(string(output), "\n", 2)[0])
		if hasToolVersion(string(output), tool) {
			return tool.Version, "current", nil
		}
		return installed, "outdated", nil
	}
	return "", "", fmt.Errorf("unknown direct tool %q", name)
}

func (m Manager) toolLaunchersCurrent(tool Tool) (bool, error) {
	for _, name := range tool.Links {
		launcher := filepath.Join(m.Home, ".local", "bin", name)
		info, err := os.Lstat(launcher)
		if os.IsNotExist(err) || err == nil && info.Mode()&os.ModeSymlink == 0 {
			return false, nil
		}
		if err != nil {
			return false, fmt.Errorf("read %s launcher: %w", name, err)
		}
		target, err := os.Readlink(launcher)
		if err != nil {
			return false, fmt.Errorf("read %s launcher: %w", name, err)
		}
		if target != filepath.Join(m.ToolsRoot(), "bin", name) {
			return false, nil
		}
	}
	return true, nil
}

func hasToolVersion(output string, tool Tool) bool {
	wanted := tool.VersionPrefix + tool.Version
	trimmed := strings.TrimSpace(output)
	return trimmed == wanted || strings.HasPrefix(trimmed, wanted+" ") || strings.HasPrefix(trimmed, wanted+"\n")
}
