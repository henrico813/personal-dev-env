package installer

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"syscall"
	"time"

	"github.com/spf13/cobra"
	"pde-installer/internal/fsutil"
	"pde-installer/internal/run"
)

// The results update.json can hold. Only installed and current mean the
// machine runs the newest main.
const (
	updateInstalled = "installed"
	updateCurrent   = "current"
	updateSkipped   = "skipped"
	updateFailed    = "failed"
)

// Bounds each fetch try, so a stalled network cannot hold update.lock forever.
const fetchTimeout = 2 * time.Minute

// Describes the last update run, as saved in ~/.local/state/pde/update.json.
type updateRecord struct {
	Result    string `json:"result"`
	Commit    string `json:"commit"`
	Reason    string `json:"reason"`
	CheckedAt string `json:"checked_at"`
}

// The keys update reads from ~/.config/pde/config.json. Install saves
// install_path, and the owner may add update_url to fetch from elsewhere.
type updateSettings struct {
	InstallPath string `json:"install_path"`
	UpdateURL   string `json:"update_url"`
}

// Returns the update command, which a timer runs from HOME with no terminal.
func newUpdateCommand(repoRoot *string) *cobra.Command {
	return &cobra.Command{
		Use:   "update",
		Short: "Fast-forward the checkout to main and install it",
		Args:  cobra.NoArgs,
		RunE: func(command *cobra.Command, _ []string) error {
			if err := rejectUID(
				os.Geteuid(),
				command.Name(),
			); err != nil {
				return err
			}
			home, err := os.UserHomeDir()
			if err != nil {
				return fmt.Errorf(
					"resolve HOME: %w",
					err,
				)
			}
			return runUpdate(
				home,
				*repoRoot,
				run.Runner{
					Stdout: command.OutOrStdout(),
					Stderr: command.ErrOrStderr(),
				},
			)
		},
	}
}

// Holds update.lock while it moves the checkout to main, and saves the
// outcome in update.json. Returns an error unless the newest main is installed.
func runUpdate(home, flagRoot string, runner run.Runner) error {
	lock, err := acquireStateLock(
		home,
		"update.lock",
		"another update is already running",
	)
	if err != nil {
		return err
	}
	record := updateCheckout(
		home,
		flagRoot,
		runner,
	)
	record.CheckedAt = time.Now().UTC().Format(time.RFC3339)
	if err := writeStateJSON(
		home,
		filepath.Join(
			home,
			".local",
			"state",
			"pde",
			"update.json",
		),
		record,
	); err != nil {
		fmt.Fprintf(
			runner.Err(),
			"warning: save update result: %v\n",
			err,
		)
	}
	return errors.Join(
		reportUpdate(
			record,
			runner,
		),
		lock.Close(),
	)
}

// Fast-forwards the checkout to the newest main and installs it, unless
// install.json already names that commit on main. Never stashes, resets or
// forces, so a checkout off main or with its own changes is skipped.
func updateCheckout(home, flagRoot string, runner run.Runner) updateRecord {
	config, settings, err := loadUpdateConfig(
		home,
		flagRoot,
	)
	if err != nil {
		return failUpdate(
			"",
			err,
		)
	}
	reason, err := findSkipReason(
		config.RepoRoot,
		runner,
	)
	if err != nil {
		return failUpdate(
			"",
			err,
		)
	}
	if reason != "" {
		return updateRecord{
			Result: updateSkipped,
			Reason: reason,
		}
	}
	url := settings.UpdateURL
	if url == "" {
		url, err = readMainRemoteURL(
			config.RepoRoot,
			runner,
		)
		if err != nil {
			return failUpdate(
				"",
				err,
			)
		}
	}
	commit, err := fetchMain(
		config.RepoRoot,
		url,
		runner,
	)
	if err != nil {
		return failUpdate(
			"",
			err,
		)
	}
	if _, err := runner.Query(
		"compare the checkout with main",
		makeGitCommand(
			config.RepoRoot,
			"merge-base",
			"--is-ancestor",
			"HEAD",
			commit,
		),
	); err != nil {
		return updateRecord{
			Result: updateSkipped,
			Commit: commit,
			Reason: "the checkout has commits that main does not have",
		}
	}
	if _, err := runner.Query(
		"fast-forward the checkout to main",
		makeGitCommand(
			config.RepoRoot,
			"merge",
			"--ff-only",
			"--quiet",
			commit,
		),
	); err != nil {
		return failUpdate(
			commit,
			err,
		)
	}
	installed, err := readInstallRecord(config.Home)
	if err == nil && installed.Commit == commit && installed.Branch == "main" {
		return updateRecord{
			Result: updateCurrent,
			Commit: commit,
		}
	}
	if err := installWithNewInstaller(
		config,
		runner,
	); err != nil {
		return failUpdate(
			commit,
			err,
		)
	}
	return updateRecord{
		Result: updateInstalled,
		Commit: commit,
	}
}

// Prints where main is when the update left it installed, and otherwise
// returns the result and reason as the error.
func reportUpdate(record updateRecord, runner run.Runner) error {
	if record.Result != updateInstalled && record.Result != updateCurrent {
		return fmt.Errorf(
			"update %s: %s",
			record.Result,
			record.Reason,
		)
	}
	_, err := fmt.Fprintf(
		runner.Out(),
		"update %s: main is at %s\n",
		record.Result,
		record.Commit,
	)
	return err
}

// Returns a failed result that keeps the error's text as its reason.
func failUpdate(commit string, err error) updateRecord {
	return updateRecord{
		Result: updateFailed,
		Commit: commit,
		Reason: err.Error(),
	}
}

// Returns the install config for the checkout to update and the update keys
// of config.json. A timer starts in HOME, so the current folder never counts.
func loadUpdateConfig(home, flagRoot string) (config, updateSettings, error) {
	settings, err := readUpdateSettings(home)
	if err != nil {
		return config{}, updateSettings{}, err
	}
	for _, candidate := range []string{
		flagRoot,
		os.Getenv("PDE_REPO_ROOT"),
		settings.InstallPath,
	} {
		if candidate == "" {
			continue
		}
		root, ok := repository(candidate)
		if !ok {
			return config{}, updateSettings{}, fmt.Errorf(
				"%s is not a personal-dev-env checkout",
				candidate,
			)
		}
		cfg, err := detectConfig(root)
		return cfg, settings, err
	}
	return config{}, updateSettings{}, errors.New("no checkout to update; pass --repo-root or run pde-installer install from one")
}

// Returns the update keys of config.json, which are empty when the file is
// missing or empty.
func readUpdateSettings(home string) (updateSettings, error) {
	var settings updateSettings
	data, err := os.ReadFile(filepath.Join(
		home,
		".config",
		"pde",
		"config.json",
	))
	if os.IsNotExist(err) || err == nil && len(bytes.TrimSpace(data)) == 0 {
		return settings, nil
	}
	if err != nil {
		return settings, fmt.Errorf(
			"read PDE config: %w",
			err,
		)
	}
	if err := json.Unmarshal(
		data,
		&settings,
	); err != nil {
		return updateSettings{}, fmt.Errorf(
			"read PDE config: %w",
			err,
		)
	}
	return settings, nil
}

// Returns why the checkout must stay where it is, or "" when it is on main
// with no changes to tracked files. Git refuses a fast-forward that would
// overwrite an untracked file, so untracked files do not count.
func findSkipReason(root string, runner run.Runner) (string, error) {
	branch := readCheckoutRevision(
		root,
		runner,
		"--abbrev-ref",
		"HEAD",
	)
	if branch != "main" {
		return fmt.Sprintf(
			"the checkout is on %s, not main",
			branch,
		), nil
	}
	status, err := runner.Query(
		"read checkout status",
		makeGitCommand(
			root,
			"status",
			"--porcelain",
			"--untracked-files=no",
		),
	)
	if err != nil {
		return "", err
	}
	if len(bytes.TrimSpace(status)) > 0 {
		return "the checkout has changes to tracked files", nil
	}
	return "", nil
}

// Returns the HTTPS form of the URL of the remote that main tracks, or of
// origin when main tracks none.
func readMainRemoteURL(root string, runner run.Runner) (string, error) {
	remote := "origin"
	output, err := runner.Query(
		"read the remote of main",
		makeGitCommand(
			root,
			"config",
			"--get",
			"branch.main.remote",
		),
	)
	if err == nil {
		remote = strings.TrimSpace(string(output))
	}
	output, err = runner.Query(
		"read the URL of "+remote,
		makeGitCommand(
			root,
			"remote",
			"get-url",
			remote,
		),
	)
	if err != nil {
		return "", err
	}
	return toHTTPSURL(strings.TrimSpace(string(output))), nil
}

// Returns the HTTPS form of a GitHub SSH address, and any other address as it
// is. The repository is public, and a timer has no SSH agent to unlock a key.
func toHTTPSURL(url string) string {
	for _, prefix := range []string{
		"git@github.com:",
		"ssh://git@github.com/",
	} {
		if path, ok := strings.CutPrefix(
			url,
			prefix,
		); ok {
			return "https://github.com/" + path
		}
	}
	return url
}

// Fetches main from the given address and returns its commit. Each of the
// two tries stops after fetchTimeout, and git never waits for a password.
func fetchMain(root, url string, runner run.Runner) (string, error) {
	if err := runner.Retry(
		"fetch main",
		2,
		func() error {
			ctx, cancel := context.WithTimeout(
				context.Background(),
				fetchTimeout,
			)
			defer cancel()
			fetch := exec.CommandContext(
				ctx,
				"git",
				"-C",
				root,
				"fetch",
				"--quiet",
				"--no-tags",
				url,
				"main",
			)
			fetch.Env = append(
				os.Environ(),
				"GIT_TERMINAL_PROMPT=0",
			)
			// A git helper can outlive a killed git and keep its output open.
			fetch.WaitDelay = time.Second
			output, err := fetch.CombinedOutput()
			if ctx.Err() != nil {
				return fmt.Errorf(
					"fetch main from %s: no answer within %s",
					url,
					fetchTimeout,
				)
			}
			if err != nil {
				return fmt.Errorf(
					"fetch main from %s: %s",
					url,
					describeFailure(
						output,
						err,
					),
				)
			}
			return nil
		},
	); err != nil {
		return "", err
	}
	output, err := runner.Query(
		"read the fetched main",
		makeGitCommand(
			root,
			"rev-parse",
			"FETCH_HEAD",
		),
	)
	if err != nil {
		return "", err
	}
	return strings.TrimSpace(string(output)), nil
}

// Builds pde-installer from the checkout, installs with it, and then moves it
// to ~/.local/bin. A failed build or install keeps the old installer.
func installWithNewInstaller(config config, runner run.Runner) error {
	stage := filepath.Join(
		config.Home,
		".local",
		"state",
		"pde",
		"update-stage",
	)
	staged := filepath.Join(
		stage,
		"pde-installer",
	)
	target := filepath.Join(
		config.LocalBin,
		"pde-installer",
	)
	if err := fsutil.GuardHome(
		config.Home,
		stage,
		staged,
		target,
	); err != nil {
		return err
	}
	if err := os.MkdirAll(
		stage,
		0o700,
	); err != nil {
		return err
	}
	defer os.RemoveAll(stage)
	// A timer starts with a short PATH, and go is linked in ~/.local/bin.
	path := "PATH=" + config.LocalBin + string(os.PathListSeparator) + os.Getenv("PATH")
	if _, err := runner.Query(
		"build pde-installer",
		run.Command{
			Name: "go",
			Args: []string{
				"build",
				"-mod=readonly",
				"-o",
				staged,
				".",
			},
			Dir: filepath.Join(
				config.RepoRoot,
				"pde-installer",
			),
			Env: []string{path},
		},
	); err != nil {
		return err
	}
	install := exec.Command(
		staged,
		"install",
		"--repo-root",
		config.RepoRoot,
	)
	// A second PATH entry would lose to the first in programs that read the
	// environment through libc, so drop the inherited one.
	for _, entry := range os.Environ() {
		if !strings.HasPrefix(entry, "PATH=") {
			install.Env = append(install.Env, entry)
		}
	}
	install.Env = append(install.Env, path)
	var stderr bytes.Buffer
	install.Stdout = runner.Out()
	install.Stderr = io.MultiWriter(
		runner.Err(),
		&stderr,
	)
	// With no terminal, a sudo password prompt fails at once instead of
	// waiting for nobody, and install skips its OpenCode restart question.
	install.SysProcAttr = &syscall.SysProcAttr{Setsid: true}
	if err := install.Run(); err != nil {
		return fmt.Errorf(
			"install: %s",
			describeFailure(
				stderr.Bytes(),
				err,
			),
		)
	}
	if err := os.Rename(
		staged,
		target,
	); err != nil {
		return fmt.Errorf(
			"replace pde-installer: %w",
			err,
		)
	}
	return nil
}

// Returns a git command that runs in root.
func makeGitCommand(root string, args ...string) run.Command {
	return run.Command{
		Name: "git",
		Args: append(
			[]string{
				"-C",
				root,
			},
			args...,
		),
	}
}

// Returns the last line of a failed program's output, where git and install
// name the cause, or the error's text when the program printed nothing.
func describeFailure(output []byte, err error) string {
	lines := strings.Split(
		strings.TrimSpace(string(output)),
		"\n",
	)
	if last := lines[len(lines)-1]; last != "" {
		return last
	}
	return err.Error()
}
