package main

import (
	"fmt"
	"sort"
	"strings"

	"github.com/spf13/cobra"
)

type Installer interface {
	Install(*Config, Runner) error
}

type installerFunc func(*Config, Runner) error

func (fn installerFunc) Install(cfg *Config, runner Runner) error {
	return fn(cfg, runner)
}

var installTargets = map[string]Installer{
	"ai-tools": installerFunc(installAITools),
	"config":   installerFunc(installConfig),
	"obsidian": installerFunc(installObsidian),
}

func newRootCmd() *cobra.Command {
	root := &cobra.Command{
		Use:   "pde",
		Short: "Install PDE targets and config sets",
	}
	root.AddCommand(newInstallCmd())
	return root
}

func newInstallCmd() *cobra.Command {
	var repoRoot string
	var dryRun bool

	cmd := &cobra.Command{
		Use:   "install <target>",
		Short: "Install a named PDE target or config set",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			cfg, err := detectConfig(repoRoot)
			if err != nil {
				return err
			}

			targetName := args[0]
			target, ok := installTargets[targetName]
			if !ok {
				return fmt.Errorf("unknown install target %q (known: %s)", targetName, strings.Join(sortedInstallTargets(), ", "))
			}

			runner := Runner{
				DryRun: dryRun,
				Stdout: cmd.OutOrStdout(),
				Stderr: cmd.ErrOrStderr(),
			}

			return target.Install(cfg, runner)
		},
	}

	cmd.Flags().StringVar(&repoRoot, "repo-root", "", "Path to the personal-dev-env checkout")
	cmd.Flags().BoolVar(&dryRun, "dry-run", false, "Print actions without writing")
	return cmd
}

func sortedInstallTargets() []string {
	names := make([]string, 0, len(installTargets))
	for name := range installTargets {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}
