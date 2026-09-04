package installer

import (
	"errors"
	"fmt"
	"os"

	"github.com/spf13/cobra"
	"pde-installer/internal/aqua"
	"pde-installer/internal/builds"
	chezmoibackend "pde-installer/internal/chezmoi"
	"pde-installer/internal/direct"
	"pde-installer/internal/fsutil"
	"pde-installer/internal/manifest"
	"pde-installer/internal/npm"
	"pde-installer/internal/run"
	"pde-installer/internal/tmux"
	"pde-installer/internal/ubuntu"
)

// NewCommand builds the pde-installer command tree.
func NewCommand() *cobra.Command {
	var repoRoot string
	root := &cobra.Command{
		Use: "pde-installer", Short: "Reconcile the PDE development environment",
		Args: cobra.NoArgs, SilenceErrors: true, SilenceUsage: true,
		RunE: func(command *cobra.Command, _ []string) error { return command.Help() },
	}
	root.CompletionOptions.DisableDefaultCmd = true
	root.PersistentFlags().StringVar(&repoRoot, "repo-root", "", "personal-dev-env checkout")
	root.AddCommand(mutatingCommand("install", "Install all pinned PDE components", &repoRoot, reconcile))
	root.AddCommand(mutatingCommand("update", "Reconcile installed components to repository pins", &repoRoot, reconcile))
	root.AddCommand(mutatingCommand("config", "Migrate legacy state and apply the chezmoi source", &repoRoot, applyConfig))
	root.AddCommand(readCommand("doctor", "Check host prerequisites and managed paths", &repoRoot, doctor))
	root.AddCommand(readCommand("list", "List ownership and installed state", &repoRoot, list))
	return root
}

func mutatingCommand(name, description string, repoRoot *string, action func(config, run.Runner) error) *cobra.Command {
	var dryRun bool
	command := &cobra.Command{
		Use: name, Short: description, Args: cobra.NoArgs,
		RunE: func(command *cobra.Command, _ []string) error {
			if err := rejectUID(os.Geteuid(), name); err != nil {
				return err
			}
			if err := manifest.Validate(); err != nil {
				return err
			}
			config, err := detectConfig(*repoRoot)
			if err != nil {
				return err
			}
			runner := run.Runner{DryRun: dryRun, ReadOnlyDryRun: dryRun && name == "config", Stdout: command.OutOrStdout(), Stderr: command.ErrOrStderr()}
			if dryRun {
				return action(config, runner)
			}
			lock, err := acquireInstallerLock(config.Home)
			if err != nil {
				return err
			}
			if err := fsutil.RecoverJournals(fsutil.JournalConfig{Home: config.Home}); err != nil {
				return errors.Join(err, lock.Close())
			}
			return errors.Join(action(config, runner), lock.Close())
		},
	}
	command.Flags().BoolVar(&dryRun, "dry-run", false, "preview ordered actions without making changes")
	return command
}

func rejectUID(uid int, command string) error {
	if uid == 0 {
		return fmt.Errorf("%s refuses UID 0; run as an unprivileged user", command)
	}
	return nil
}

func readCommand(name, description string, repoRoot *string, action func(config, run.Runner) error) *cobra.Command {
	return &cobra.Command{
		Use: name, Short: description, Args: cobra.NoArgs,
		RunE: func(command *cobra.Command, _ []string) error {
			config, err := detectConfig(*repoRoot)
			if err != nil {
				return err
			}
			return action(config, run.Runner{Stdout: command.OutOrStdout(), Stderr: command.ErrOrStderr()})
		},
	}
}

func reconcile(config config, runner run.Runner) error {
	// APT owns system dependencies. Later stages journal changes below HOME.
	if err := ubuntu.New(runner).Reconcile(); err != nil {
		return fmt.Errorf("Ubuntu packages: %w", err)
	}
	if err := hostPreflight(config, runner, preflightQuiet); err != nil {
		return err
	}
	// Keep successful stages reversible until every stage succeeds.
	var journals []*fsutil.Journal
	fail := func(stage string, err error) error {
		failures := []error{err}
		for i := len(journals) - 1; i >= 0; i-- {
			if rollbackErr := journals[i].Rollback(); rollbackErr != nil {
				failures = append(failures, fmt.Errorf("rollback: %w", rollbackErr))
			}
		}
		return fmt.Errorf("%s: %w", stage, errors.Join(failures...))
	}
	tmuxJournal, err := tmux.New(config.Home, runner).Reconcile()
	if err != nil {
		return fail("tmux", err)
	}
	journals = append(journals, tmuxJournal)
	// Order matters: runtimes precede their package tools, and config precedes
	// builds that use files installed by chezmoi.
	aquaManager := aqua.New(config.Home, config.RepoRoot, runner)
	aquaJournal, err := aquaManager.Reconcile()
	if err != nil {
		return fail("Aqua", err)
	}
	journals = append(journals, aquaJournal)
	directManager := direct.New(config.Home, runner)
	toolJournal, err := directManager.ReconcileTools()
	if err != nil {
		return fail("direct tools", err)
	}
	journals = append(journals, toolJournal)
	npmJournal, err := npm.New(config.Home, config.RepoRoot, runner).Reconcile()
	if err != nil {
		return fail("npm", err)
	}
	journals = append(journals, npmJournal)
	directJournal, err := directManager.Reconcile()
	if err != nil {
		return fail("direct artifacts", err)
	}
	journals = append(journals, directJournal)
	buildManager := builds.New(config.Home, config.RepoRoot, runner)
	buildJournal, err := buildManager.Reconcile()
	if err != nil {
		return fail("local builds", err)
	}
	journals = append(journals, buildJournal)
	migrationJournal, err := prepareLegacyConfig(config, runner)
	if err != nil {
		return fail("PDE config migration", err)
	}
	journals = append(journals, migrationJournal)
	chezmoiJournal, err := chezmoibackend.New(config.Home, config.RepoRoot, config.AquaRoot, runner).Apply()
	if err != nil {
		return fail("chezmoi", err)
	}
	journals = append(journals, chezmoiJournal)
	blinkJournal, err := buildManager.BuildBlink()
	if err != nil {
		return fail("blink.cmp", err)
	}
	journals = append(journals, blinkJournal)
	if err := fsutil.CommitJournals(journals...); err != nil {
		return fmt.Errorf("clean successful backups: %w", err)
	}
	return nil
}

func applyConfig(config config, runner run.Runner) error {
	migrationJournal, err := prepareLegacyConfig(config, runner)
	if err != nil {
		return err
	}
	journal, err := chezmoibackend.New(config.Home, config.RepoRoot, config.AquaRoot, runner).Apply()
	if err != nil {
		return migrationJournal.Revert(err)
	}
	return fsutil.CommitJournals(migrationJournal, journal)
}
