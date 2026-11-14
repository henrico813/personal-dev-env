package ubuntu

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"pde-installer/internal/run"
)

// Ubuntu's zsh package installs here. A dry run skips apt, so zsh may not exist yet.
const packagedZsh = "/usr/bin/zsh"

// Makes zsh the login shell of username, so new terminals load the managed
// ~/.zshrc. Leaves any zsh alone, and only warns when /etc/shells leaves out
// the zsh on PATH.
func (m Manager) SetLoginShell(username string) error {
	// $SHELL keeps the old shell until the next login, so read the passwd entry.
	entry, err := m.runner.Query("read login shell", run.Command{Name: "getent", Args: []string{"passwd", username}})
	if err != nil {
		return err
	}
	fields := strings.Split(strings.TrimSpace(string(entry)), ":")
	if len(fields) != 7 {
		return fmt.Errorf("parse passwd entry of %s: %q", username, entry)
	}
	current := fields[6]
	// Any zsh counts, so /bin/zsh stays when PATH finds /usr/bin/zsh.
	if filepath.Base(current) == "zsh" {
		return nil
	}
	zsh, err := run.LookPath("zsh", os.Getenv("PATH"))
	switch {
	case err != nil && m.runner.DryRun:
		zsh = packagedZsh
	case err != nil:
		return err
	default:
		listed, err := isListedShell(m.shellsPath, zsh)
		if err != nil {
			return err
		}
		// chsh and PAM refuse a shell missing from this file.
		if !listed {
			fmt.Fprintf(m.runner.Err(), "warning: %s is not in %s, so the login shell stays %s; add it there or put a listed zsh first on PATH\n", zsh, m.shellsPath, current)
			return nil
		}
	}
	// Unlike chsh, usermod does not ask for the user's own password. sudo may prompt as it does for apt-get.
	return m.runner.Run("set login shell to zsh", run.Command{Name: "sudo", Args: []string{"usermod", "-s", zsh, username}})
}

// Reports whether shell is one of the paths listed in the shells file.
func isListedShell(shellsPath, shell string) (bool, error) {
	data, err := os.ReadFile(shellsPath)
	if err != nil {
		return false, fmt.Errorf("read login shells: %w", err)
	}
	for _, line := range strings.Split(string(data), "\n") {
		if strings.TrimSpace(line) == shell {
			return true, nil
		}
	}
	return false, nil
}
