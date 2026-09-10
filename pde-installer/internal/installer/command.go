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
	"pde-installer/internal/profile"
	"pde-installer/internal/run"
	"pde-installer/internal/tmux"
	"pde-installer/internal/ubuntu"
)

// NewCommand builds the pde-installer command tree.
func NewCommand() *cobra.Command {
	var repoRoot string
	root := &cobra.Command{
		Use: "pde-installer", Short: "Reconcile the PDE development environment",
		Long: `Install and maintain the PDE development environment.

Use config after changing managed home configuration. Use update after changing
tools, packages, runtimes, or local builds; it also applies home configuration.`,
		Args: cobra.NoArgs, SilenceErrors: true, SilenceUsage: true,
		RunE: func(command *cobra.Command, _ []string) error { return command.Help() },
	}
	root.CompletionOptions.DisableDefaultCmd = true
	root.PersistentFlags().StringVar(&repoRoot, "repo-root", "", "personal-dev-env checkout")
	var requestedProfile string
	install := mutatingCommand("install", "Install pinned PDE components", &repoRoot, installProfile, &requestedProfile, reconcile)
	install.Flags().StringVar(&requestedProfile, "profile", "", "installation profile: full or terminal")
	root.AddCommand(install)
	update := mutatingCommand("update", "Update saved-profile tools and home configuration", &repoRoot, requireProfile, nil, reconcile)
	update.Long = `Update managed tools and home configuration for the saved profile.

Use this after pulling changes to package lists, tool versions, runtimes, or
local builds. It also applies managed home configuration.

Use config instead when only managed home configuration changed. A saved profile
is required. Do not run this command as root.`
	update.Example = `  pde-installer update --dry-run
  pde-installer update`
	root.AddCommand(update)

	config := mutatingCommand("config", "Apply saved-profile home configuration", &repoRoot, requireProfile, nil, applyConfig)
	config.Long = `Apply managed home configuration for the saved profile.

Use this after pulling changes only to shell, Git, editor, or AI configuration.
It does not update tools, runtimes, packages, or local builds. A normal run can
update managed configuration files and run source-managed scripts.

A saved profile and the chezmoi binary installed by PDE are required. Do not run
this command as root.`
	config.Example = `  pde-installer config --dry-run
  pde-installer config`
	root.AddCommand(config)
	root.AddCommand(readCommand("doctor", "Check host prerequisites and managed paths", &repoRoot, doctor))
	root.AddCommand(readCommand("list", "List ownership and installed state", &repoRoot, list))
	return root
}

func mutatingCommand(name, description string, repoRoot *string, mode profileMode, requested *string, action func(config, run.Runner) error) *cobra.Command {
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
				pending, err := fsutil.HasPendingJournals(fsutil.JournalConfig{Home: config.Home})
				if err != nil {
					return err
				}
				if pending {
					return fmt.Errorf("pending filesystem recovery; rerun without --dry-run")
				}
				requestedValue := ""
				if requested != nil {
					requestedValue = *requested
				}
				config.Profile, err = resolveProfile(config.Home, requestedValue, mode)
				if err != nil {
					return err
				}
				return action(config, runner)
			}
			lock, err := acquireInstallerLock(config.Home)
			if err != nil {
				return err
			}
			if err := fsutil.RecoverJournals(fsutil.JournalConfig{Home: config.Home}); err != nil {
				return errors.Join(err, lock.Close())
			}
			requestedValue := ""
			if requested != nil {
				requestedValue = *requested
			}
			config.Profile, err = resolveProfile(config.Home, requestedValue, mode)
			if err != nil {
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
			lock, err := acquireInstallerLock(config.Home)
			if err != nil {
				return err
			}
			if err := fsutil.RecoverJournals(fsutil.JournalConfig{Home: config.Home}); err != nil {
				return errors.Join(err, lock.Close())
			}
			config.Profile, err = resolveProfile(config.Home, "", readProfile)
			if err != nil {
				return errors.Join(err, lock.Close())
			}
			return errors.Join(action(config, run.Runner{Stdout: command.OutOrStdout(), Stderr: command.ErrOrStderr()}), lock.Close())
		},
	}
}

func reconcile(config config, runner run.Runner) error {
	if err := config.validateProfile(); err != nil {
		return err
	}
	// APT owns system dependencies. Later stages journal changes below HOME.
	if err := ubuntu.New(config.Profile, runner).Reconcile(); err != nil {
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
	var buildManager builds.Manager
	var directManager direct.Manager
	if config.Profile == profile.Full {
		directManager = direct.New(config.Home, runner)
		toolJournal, err := directManager.ReconcileTools()
		if err != nil {
			return fail("direct tools", err)
		}
		journals = append(journals, toolJournal)
	}
	aquaManager := aqua.New(config.Home, config.RepoRoot, config.Profile, runner)
	aquaJournal, err := aquaManager.Reconcile()
	if err != nil {
		return fail("Aqua", err)
	}
	journals = append(journals, aquaJournal)
	if config.Profile == profile.Full {
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
		buildManager = builds.New(config.Home, config.RepoRoot, runner)
		buildJournal, err := buildManager.Reconcile()
		if err != nil {
			return fail("local builds", err)
		}
		journals = append(journals, buildJournal)
	}
	migrationJournal, err := prepareLegacyConfig(config, runner)
	if err != nil {
		return fail("PDE config migration", err)
	}
	journals = append(journals, migrationJournal)
	chezmoiJournal, err := chezmoibackend.New(config.Home, config.RepoRoot, config.AquaRoot, config.Profile, runner).Apply()
	if err != nil {
		return fail("chezmoi", err)
	}
	journals = append(journals, chezmoiJournal)
	if config.Profile == profile.Full {
		blinkJournal, err := buildManager.BuildBlink()
		if err != nil {
			return fail("blink.cmp", err)
		}
		journals = append(journals, blinkJournal)
	}
	if err := fsutil.CommitJournals(journals...); err != nil {
		return fmt.Errorf("clean successful backups: %w", err)
	}
	return nil
}

func applyConfig(config config, runner run.Runner) error {
	if err := config.validateProfile(); err != nil {
		return err
	}
	migrationJournal, err := prepareLegacyConfig(config, runner)
	if err != nil {
		return err
	}
	journal, err := chezmoibackend.New(config.Home, config.RepoRoot, config.AquaRoot, config.Profile, runner).Apply()
	if err != nil {
		return migrationJournal.Revert(err)
	}
	return fsutil.CommitJournals(migrationJournal, journal)
}
