package installer

import (
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/spf13/cobra"
	"pde-installer/internal/fsutil"
	"pde-installer/internal/run"
)

// Describes the personal-dev-env build of the last install that put every
// component in place. Commit and Branch hold "unknown" when git could not
// read them.
type installRecord struct {
	Commit      string `json:"commit"`
	Branch      string `json:"branch"`
	InstalledAt string `json:"installed_at"`
}

// Returns the path of install.json. It sits under HOME, so version works
// from any folder.
func installRecordPath(home string) string {
	return filepath.Join(
		home,
		".local",
		"state",
		"pde",
		"install.json",
	)
}

// Returns the build recorded by the last install after every component was
// in place. The error wraps fs.ErrNotExist when this machine has no record.
func readInstallRecord(home string) (installRecord, error) {
	path := installRecordPath(home)
	data, err := os.ReadFile(path)
	if err != nil {
		return installRecord{}, err
	}
	var record installRecord
	if err := json.Unmarshal(
		data,
		&record,
	); err != nil {
		return installRecord{}, fmt.Errorf(
			"read %s: %w",
			path,
			err,
		)
	}
	return record, nil
}

// Saves the checkout's commit and branch with the install time. A dry run
// saves nothing, and a failed save only warns because the install is done.
func recordInstall(config config, runner run.Runner, now time.Time) {
	if runner.DryRun {
		return
	}
	record := installRecord{
		Commit: readCheckoutRevision(
			config.RepoRoot,
			runner,
			"HEAD",
		),
		Branch: readCheckoutRevision(
			config.RepoRoot,
			runner,
			"--abbrev-ref",
			"HEAD",
		),
		InstalledAt: now.UTC().Format(time.RFC3339),
	}
	if err := writeInstallRecord(
		config.Home,
		record,
	); err != nil {
		fmt.Fprintf(
			runner.Err(),
			"warning: record installed version: %v; pde-installer version may show the previous install\n",
			err,
		)
	}
}

// Warns on stderr when an install from main will replace an install from
// another branch, so a branch under test is not lost unnoticed. A missing or
// unreadable record names no branch, so nothing prints.
func warnReplacingBranch(config config, runner run.Runner) {
	record, err := readInstallRecord(config.Home)
	if err != nil || record.Branch == "main" {
		return
	}
	branch := readCheckoutRevision(
		config.RepoRoot,
		runner,
		"--abbrev-ref",
		"HEAD",
	)
	if branch != "main" {
		return
	}
	fmt.Fprintf(
		runner.Err(),
		"warning: replacing the install from branch %s (commit %s, installed %s) with main\n",
		record.Branch,
		record.Commit,
		record.InstalledAt,
	)
}

// Returns what git rev-parse prints for the checkout, or "unknown" when git
// fails. A detached checkout gives HEAD for its branch.
func readCheckoutRevision(repoRoot string, runner run.Runner, args ...string) string {
	output, err := runner.Query(
		"read checkout version",
		run.Command{
			Name: "git",
			Args: append(
				[]string{
					"-C",
					repoRoot,
					"rev-parse",
				},
				args...,
			),
		},
	)
	value := strings.TrimSpace(string(output))
	if err != nil || value == "" {
		return "unknown"
	}
	return value
}

func writeInstallRecord(home string, record installRecord) error {
	return writeStateJSON(
		home,
		installRecordPath(home),
		record,
	)
}

// Saves a value as indented JSON at a path within HOME. The file is replaced
// whole, so a reader never sees half a record.
func writeStateJSON(home, path string, value any) error {
	if err := fsutil.GuardHome(
		home,
		path,
	); err != nil {
		return err
	}
	if err := os.MkdirAll(
		filepath.Dir(path),
		0o700,
	); err != nil {
		return err
	}
	data, err := json.MarshalIndent(
		value,
		"",
		"  ",
	)
	if err != nil {
		return err
	}
	return fsutil.WriteFileAtomic(
		path,
		append(
			data,
			'\n',
		),
	)
}

// Returns the version command. It reads only the install record, so it needs
// no checkout and does not wait for a running install.
func newVersionCommand() *cobra.Command {
	return &cobra.Command{
		Use:   "version",
		Short: "Print the commit, branch and time of the installed build",
		Args:  cobra.NoArgs,
		RunE: func(command *cobra.Command, _ []string) error {
			home, err := os.UserHomeDir()
			if err != nil {
				return fmt.Errorf(
					"resolve HOME: %w",
					err,
				)
			}
			record, err := readInstallRecord(home)
			if errors.Is(
				err,
				fs.ErrNotExist,
			) {
				return fmt.Errorf("no install recorded on this machine; run pde-installer install first")
			}
			if err != nil {
				return err
			}
			_, err = fmt.Fprintf(
				command.OutOrStdout(),
				"commit %s\nbranch %s\ninstalled %s\n",
				record.Commit,
				record.Branch,
				record.InstalledAt,
			)
			return err
		},
	}
}
