package direct

import (
	"fmt"
	"os"
	"path/filepath"

	"pde-installer/internal/fsutil"
	"pde-installer/internal/manifest"
	"pde-installer/internal/run"
)

// Font identifies a pinned Nerd Fonts release artifact.
type Font struct {
	Name, Archive, SHA256 string
}

// Fonts returns the artifacts owned by the direct backend.
func Fonts() []Font {
	return []Font{
		{Name: "FiraCode", Archive: "FiraCode.zip", SHA256: "4ee8fbafecfc90460399b9828270b8ece30ccbf60b3ab875d64ff77696c6e262"},
		{Name: "JetBrainsMono", Archive: "JetBrainsMono.tar.xz", SHA256: "6cf8822bc1ca18e34b06578c7499f380c019e6ffc883eed26df5f498dfcc4006"},
	}
}

// Manager installs direct-release artifacts for one user.
type Manager struct {
	Home   string
	Runner run.Runner
}

// New returns a direct artifact manager.
func New(home string, runner run.Runner) Manager {
	return Manager{Home: home, Runner: runner}
}

// Reconcile installs missing or outdated managed fonts.
func (m Manager) Reconcile() (*fsutil.Journal, error) {
	allCurrent := true
	for _, font := range Fonts() {
		current, err := m.current(font)
		if err != nil {
			return nil, err
		}
		if !current {
			allCurrent = false
		}
	}
	if allCurrent {
		return &fsutil.Journal{}, nil
	}
	if m.Runner.DryRun {
		for _, font := range Fonts() {
			current, err := m.current(font)
			if err != nil {
				return nil, err
			}
			if current {
				continue
			}
			item, _ := manifest.Find(font.Name, manifest.Direct)
			url := "https://github.com/ryanoasis/nerd-fonts/releases/download/" + item.Version + "/" + font.Archive
			if err := m.Runner.Plan("download and verify "+url+" sha256="+font.SHA256, nil); err != nil {
				return nil, err
			}
		}
		if err := m.Runner.Plan("atomically activate managed fonts", nil); err != nil {
			return nil, err
		}
		if err := m.Runner.Plan("refresh font cache", nil); err != nil {
			return nil, err
		}
		return &fsutil.Journal{}, nil
	}
	parent := filepath.Join(m.Home, ".local", "share", "fonts", "pde")
	if err := fsutil.GuardHome(m.Home, filepath.Dir(parent), parent); err != nil {
		return nil, err
	}
	if err := os.MkdirAll(filepath.Dir(parent), 0o755); err != nil {
		return nil, err
	}
	workspace, err := os.MkdirTemp(filepath.Dir(parent), ".pde-fonts-")
	if err != nil {
		return nil, err
	}
	tracked := false
	defer func() {
		if !tracked {
			_ = os.RemoveAll(workspace)
		}
	}()
	stage := filepath.Join(workspace, "fonts")
	if _, err := os.Stat(parent); err == nil {
		if err := fsutil.CopyTree(parent, stage); err != nil {
			return nil, err
		}
	} else if !os.IsNotExist(err) {
		return nil, err
	}
	if err := os.MkdirAll(stage, 0o755); err != nil {
		return nil, err
	}
	for _, font := range Fonts() {
		current, err := m.current(font)
		if err != nil {
			return nil, err
		}
		if current {
			continue
		}
		item, _ := manifest.Find(font.Name, manifest.Direct)
		url := "https://github.com/ryanoasis/nerd-fonts/releases/download/" + item.Version + "/" + font.Archive
		archive := filepath.Join(workspace, font.Archive)
		extracted := filepath.Join(stage, font.Name)
		if err := fsutil.GuardHome(m.Home, workspace, archive, extracted); err != nil {
			return nil, err
		}
		if err := m.Runner.Retry("font download "+font.Name, 3, func() error {
			if err := fsutil.GuardHome(m.Home, workspace, archive); err != nil {
				return err
			}
			return fsutil.Download(url, archive, font.SHA256)
		}); err != nil {
			return nil, err
		}
		if err := os.RemoveAll(extracted); err != nil {
			return nil, err
		}
		if err := os.MkdirAll(extracted, 0o755); err != nil {
			return nil, err
		}
		if filepath.Ext(font.Archive) == ".zip" {
			if err := fsutil.ExtractZip(archive, extracted); err != nil {
				return nil, err
			}
		} else if err := m.Runner.Run("extract "+font.Name, run.Command{Name: "tar", Args: []string{"-xJf", archive, "-C", extracted}}); err != nil {
			return nil, err
		}
		if err := os.WriteFile(filepath.Join(extracted, ".pde-checksum"), []byte(font.SHA256+"\n"), 0o644); err != nil {
			return nil, err
		}
	}
	journal, err := fsutil.NewJournal(fsutil.JournalConfig{Home: m.Home})
	if err != nil {
		return nil, err
	}
	if err := journal.Activate(stage, parent); err != nil {
		return nil, fmt.Errorf("activate fonts: %w", err)
	}
	if err := journal.AddCleanup(workspace); err != nil {
		return nil, journal.Revert(err)
	}
	tracked = true
	if err := m.Runner.Run("refresh font cache", run.Command{Name: "fc-cache", Args: []string{"-f"}}); err != nil {
		return nil, journal.Revert(err)
	}
	return journal, nil
}

func (m Manager) current(font Font) (bool, error) {
	data, err := os.ReadFile(filepath.Join(m.Home, ".local", "share", "fonts", "pde", font.Name, ".pde-checksum"))
	if err != nil {
		if os.IsNotExist(err) {
			return false, nil
		}
		return false, fmt.Errorf("read %s checksum: %w", font.Name, err)
	}
	return string(data) == font.SHA256+"\n", nil
}

// Status reports whether a font matches its pinned checksum.
func (m Manager) Status(font Font) string {
	status, err := m.Probe(font)
	if err != nil {
		return "error"
	}
	return status
}

// Probe reports font state without hiding filesystem errors.
func (m Manager) Probe(font Font) (string, error) {
	current, err := m.current(font)
	if err != nil {
		return "", err
	}
	if current {
		return "current", nil
	}
	return "missing", nil
}
