package homebrew

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"runtime"
	"slices"
	"strings"

	"pde-installer/internal/profile"
	"pde-installer/internal/run"
)

// Installs a profile's Homebrew formulae on an Apple Silicon Mac.
type Manager struct {
	profile profile.Profile
	desktop profile.Desktop
	runner  run.Runner
	goarch  string
}

func New(selected profile.Profile, desktop profile.Desktop, runner run.Runner) Manager {
	return Manager{profile: selected, desktop: desktop, runner: runner, goarch: runtime.GOARCH}
}

// Returns an error that names what this Mac or selection cannot install.
func (m Manager) Validate() error {
	if m.goarch != "arm64" {
		return fmt.Errorf("macOS on %s is not supported; pdev needs an Apple Silicon Mac", m.goarch)
	}
	if len(m.profile.Formulae()) == 0 {
		return fmt.Errorf("the %s profile does not install on macOS; use terminal or client", m.profile)
	}
	if m.desktop != "" {
		return fmt.Errorf("the %s desktop needs Linux; macOS installs terminal or client without one", m.desktop)
	}
	if _, err := run.LookPath("brew", os.Getenv("PATH")); err != nil {
		return fmt.Errorf("Homebrew is missing; install it from https://brew.sh (%w)", err)
	}
	return nil
}

// Installs the profile's formulae that are not installed yet.
func (m Manager) Reconcile() error {
	if err := m.Validate(); err != nil {
		return err
	}
	output, err := m.runner.Query("list Homebrew formulae", run.Command{Name: "brew", Args: []string{"list", "--formula", "-1"}})
	if err != nil {
		return err
	}
	installed := strings.Fields(string(output))
	var missing []string
	for _, name := range m.profile.Formulae() {
		if !slices.Contains(installed, name) {
			missing = append(missing, name)
		}
	}
	if len(missing) == 0 {
		return nil
	}
	return m.runner.Run("install Homebrew formulae", run.Command{Name: "brew", Args: append([]string{"install"}, missing...)})
}

// Reports the installed version of one formula and "installed", or
// "missing".
func (m Manager) Probe(name string) (string, string, error) {
	output, err := m.runner.Query("inspect Homebrew formula "+name, run.Command{Name: "brew", Args: []string{"list", "--formula", "--versions", name}})
	if err != nil {
		var exitError *exec.ExitError
		if errors.As(err, &exitError) && exitError.ExitCode() == 1 {
			return "", "missing", nil
		}
		return "", "", err
	}
	// The output looks like "git 2.47.1", with newer versions last.
	fields := strings.Fields(string(output))
	if len(fields) < 2 {
		return "", "missing", nil
	}
	return fields[len(fields)-1], "installed", nil
}
