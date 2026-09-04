package ubuntu

import (
	"errors"
	"fmt"
	"os"
	"os/exec"
	"strconv"
	"strings"

	"pde-installer/internal/run"
)

func packages() []string {
	return []string{
		"bison", "build-essential", "bzip2", "ca-certificates", "curl", "file",
		"fontconfig", "gawk", "git", "gzip", "libevent-dev", "libncurses-dev",
		"make", "patch", "pkg-config", "python3", "tar", "unzip", "xclip",
		"xz-utils", "zsh",
	}
}

type Manager struct {
	runner        run.Runner
	osReleasePath string
}

func New(runner run.Runner) Manager {
	return Manager{runner: runner, osReleasePath: "/etc/os-release"}
}

// Validate checks whether the host can install Ubuntu packages.
func (m Manager) Validate() error {
	if err := validateRelease(m.osReleasePath); err != nil {
		return err
	}
	for _, name := range []string{"apt-get", "dpkg-query", "sudo"} {
		if _, err := run.LookPath(name, os.Getenv("PATH")); err != nil {
			return fmt.Errorf("Ubuntu package manager: %w", err)
		}
	}
	return nil
}

func (m Manager) Reconcile() error {
	if err := m.Validate(); err != nil {
		return err
	}

	packageNames := packages()
	missing := make([]string, 0, len(packageNames))
	for _, name := range packageNames {
		output, err := m.runner.Query("inspect Ubuntu package "+name, run.Command{
			Name: "dpkg-query",
			Args: []string{"-W", "-f=${db:Status-Abbrev}", name},
		})
		if err != nil {
			var exitError *exec.ExitError
			if errors.As(err, &exitError) && exitError.ExitCode() == 1 {
				missing = append(missing, name)
				continue
			}
			return err
		}
		if strings.TrimSpace(string(output)) != "ii" {
			missing = append(missing, name)
		}
	}
	if len(missing) == 0 {
		return nil
	}
	if err := m.runner.Run("update Ubuntu package index", run.Command{Name: "sudo", Args: []string{"apt-get", "update"}}); err != nil {
		return err
	}
	args := append([]string{"apt-get", "install", "-y", "--no-install-recommends"}, missing...)
	return m.runner.Run("install Ubuntu dependencies", run.Command{Name: "sudo", Args: args})
}

// Probe reports the installed Ubuntu package version and state.
func (m Manager) Probe(name string) (string, string, error) {
	output, err := m.runner.Query("inspect Ubuntu package "+name, run.Command{
		Name: "dpkg-query",
		Args: []string{"-W", "-f=${db:Status-Abbrev}\t${Version}", name},
	})
	if err != nil {
		var exitError *exec.ExitError
		if errors.As(err, &exitError) && exitError.ExitCode() == 1 {
			return "", "missing", nil
		}
		return "", "", err
	}
	status, installed, _ := strings.Cut(strings.TrimSpace(string(output)), "\t")
	if strings.TrimSpace(status) != "ii" {
		return installed, "missing", nil
	}
	return installed, "installed", nil
}

func validateRelease(path string) error {
	data, err := os.ReadFile(path)
	if err != nil {
		return fmt.Errorf("read Ubuntu release: %w", err)
	}
	values, err := parseRelease(string(data))
	if err != nil {
		return err
	}
	if values["ID"] != "ubuntu" {
		return fmt.Errorf("unsupported operating system %q: Ubuntu 22.04 or newer required", values["ID"])
	}
	parts := strings.Split(values["VERSION_ID"], ".")
	if len(parts) < 2 {
		return fmt.Errorf("unsupported Ubuntu version %q: Ubuntu 22.04 or newer required", values["VERSION_ID"])
	}
	major, majorErr := strconv.Atoi(parts[0])
	minor, minorErr := strconv.Atoi(parts[1])
	if majorErr != nil || minorErr != nil || major < 22 || major == 22 && minor < 4 {
		return fmt.Errorf("unsupported Ubuntu version %q: Ubuntu 22.04 or newer required", values["VERSION_ID"])
	}
	return nil
}

func parseRelease(contents string) (map[string]string, error) {
	values := make(map[string]string)
	for _, line := range strings.Split(contents, "\n") {
		line = strings.TrimSpace(line)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}
		key, value, ok := strings.Cut(line, "=")
		if !ok {
			continue
		}
		value = strings.TrimSpace(value)
		if strings.HasPrefix(value, `"`) {
			unquoted, err := strconv.Unquote(value)
			if err != nil {
				return nil, fmt.Errorf("parse Ubuntu release %s: %w", key, err)
			}
			value = unquoted
		}
		values[strings.TrimSpace(key)] = value
	}
	return values, nil
}
