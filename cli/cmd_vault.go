package main

import (
	"errors"
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"
)

func newVaultCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "vault",
		Short: "Manage persisted PDE vault selection and lookup",
	}
	cmd.AddCommand(newVaultDefaultCmd())
	cmd.AddCommand(newVaultLocateCmd())
	return cmd
}

func newVaultDefaultCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:           "default",
		Short:         "Get or set the persisted main/work selector",
		Args:          cobra.NoArgs,
		SilenceUsage:  true,
		SilenceErrors: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			homeDir, err := os.UserHomeDir()
			if err != nil {
				return err
			}
			return runVaultDefaultGet(cmd.OutOrStdout(), homeDir)
		},
	}
	cmd.AddCommand(newVaultDefaultGetCmd())
	cmd.AddCommand(newVaultDefaultSetCmd())
	return cmd
}

func newVaultDefaultGetCmd() *cobra.Command {
	return &cobra.Command{
		Use:           "get",
		Short:         "Print the persisted main/work selector",
		Args:          cobra.NoArgs,
		SilenceUsage:  true,
		SilenceErrors: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			homeDir, err := os.UserHomeDir()
			if err != nil {
				return err
			}
			return runVaultDefaultGet(cmd.OutOrStdout(), homeDir)
		},
	}
}

func newVaultDefaultSetCmd() *cobra.Command {
	return &cobra.Command{
		Use:           "set <main|work>",
		Short:         "Persist the main/work selector",
		Args:          cobra.ExactArgs(1),
		SilenceUsage:  true,
		SilenceErrors: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			homeDir, err := os.UserHomeDir()
			if err != nil {
				return err
			}
			return runVaultDefaultSet(cmd.OutOrStdout(), homeDir, args[0])
		},
	}
}

func newVaultLocateCmd() *cobra.Command {
	var opts vaultLocateOptions

	cmd := &cobra.Command{
		Use:           "locate [reference]",
		Short:         "Locate a note in the selected PDE vault",
		Args:          cobra.MaximumNArgs(1),
		SilenceUsage:  true,
		SilenceErrors: true,
		RunE: func(cmd *cobra.Command, args []string) error {
			opts.Reference = ""
			if len(args) == 1 {
				opts.Reference = args[0]
			}
			homeDir, err := os.UserHomeDir()
			if err != nil {
				return err
			}
			return runVaultLocate(cmd.OutOrStdout(), homeDir, opts)
		},
	}

	cmd.Flags().StringVar(&opts.Vault, "vault", "default", "Vault selector: main|work|default|any; default follows PDE_DEFAULT_VAULT")
	cmd.Flags().StringVar(&opts.Filename, "filename", "", "Exact note filename to locate")
	cmd.Flags().StringVar(&opts.Query, "query", "", "Search query to locate")
	cmd.Flags().BoolVar(&opts.JSON, "json", false, "Emit JSON output")
	return cmd
}

func runVaultLocate(out io.Writer, homeDir string, opts vaultLocateOptions) error {
	opts.Filename = normalizeQueryInput(opts.Filename)
	opts.Reference = normalizeVaultReference(opts.Reference)
	opts.Query = normalizeQueryInput(opts.Query)
	opts.Vault = normalizeQueryInput(opts.Vault)

	if opts.Reference != "" && (opts.Filename != "" || opts.Query != "") {
		return writeVaultLocateError(out, opts.JSON, errors.New("reference is mutually exclusive with --filename and --query"))
	}
	if opts.Reference == "" && opts.Filename == "" && opts.Query == "" {
		return writeVaultLocateError(out, opts.JSON, errors.New("provide a reference, --filename, or --query"))
	}
	if opts.Filename != "" && opts.Query != "" {
		return writeVaultLocateError(out, opts.JSON, errors.New("--filename and --query are mutually exclusive"))
	}

	vaults, err := resolveVaultPaths(homeDir, opts.Vault)
	if err != nil {
		return writeVaultLocateError(out, opts.JSON, err)
	}

	matches, err := findVaultNotes(vaults, opts.Filename, opts.Reference, opts.Query)
	if err != nil {
		return writeVaultLocateError(out, opts.JSON, err)
	}

	switch len(matches) {
	case 0:
		return writeVaultLocateStatus(out, opts.JSON, vaultLocateResult{Status: "not_found"})
	case 1:
		return writeVaultLocateStatus(out, opts.JSON, vaultLocateResult{Status: "found", Path: matches[0]})
	default:
		if opts.JSON {
			return writeVaultLocateStatus(out, true, vaultLocateResult{Status: "ambiguous", Matches: matches})
		}
		for _, match := range matches {
			fmt.Fprintln(out, match)
		}
		return fmt.Errorf("ambiguous match")
	}
}

func writeVaultLocateError(out io.Writer, jsonMode bool, err error) error {
	if jsonMode {
		return writeVaultLocateStatus(out, true, vaultLocateResult{Status: "error", Error: err.Error()})
	}
	return err
}

func writeVaultLocateStatus(out io.Writer, jsonMode bool, result vaultLocateResult) error {
	if !jsonMode {
		switch result.Status {
		case "found":
			fmt.Fprintln(out, result.Path)
		case "not_found":
			return fmt.Errorf("no match found")
		case "ambiguous":
			for _, match := range result.Matches {
				fmt.Fprintln(out, match)
			}
			return fmt.Errorf("ambiguous match")
		case "error":
			return fmt.Errorf(result.Error)
		}
		return nil
	}
	return encodeVaultLocateJSON(out, result)
}

func runVaultDefaultGet(out io.Writer, homeDir string) error {
	state, err := readVaultState(homeDir)
	if err != nil {
		return err
	}
	selector := state.Default
	if selector == "" {
		selector = "unset"
	}
	_, err = fmt.Fprintln(out, selector)
	return err
}

func runVaultDefaultSet(out io.Writer, homeDir, selector string) error {
	selector = normalizeVaultSelector(selector)
	if selector != "main" && selector != "work" {
		return newVaultError(vaultInvalidSelector, nil, selector)
	}
	state, err := readVaultState(homeDir)
	if err != nil {
		return err
	}
	state.Default = selector
	if _, err := selectVaultPaths(state, "default"); err != nil {
		return err
	}
	if err := writeVaultState(homeDir, state); err != nil {
		return err
	}
	_, err = fmt.Fprintln(out, selector)
	return err
}
