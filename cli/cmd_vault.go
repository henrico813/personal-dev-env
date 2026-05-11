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
		Short: "Manage PDE vault tooling",
	}
	cmd.AddCommand(newVaultLocateCmd())
	return cmd
}

func newVaultLocateCmd() *cobra.Command {
	var opts vaultLocateOptions

	cmd := &cobra.Command{
		Use:           "locate [reference]",
		Short:         "Locate a note in a PDE vault",
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
			return runVaultLocate(cmd.OutOrStdout(), homeDir, os.LookupEnv, opts)
		},
	}

	cmd.Flags().StringVar(&opts.Vault, "vault", "default", "Vault selector: main|work|default|any")
	cmd.Flags().StringVar(&opts.Filename, "filename", "", "Exact note filename to locate")
	cmd.Flags().StringVar(&opts.Query, "query", "", "Search query to locate")
	cmd.Flags().BoolVar(&opts.JSON, "json", false, "Emit JSON output")
	return cmd
}

func runVaultLocate(out io.Writer, homeDir string, lookup envLookup, opts vaultLocateOptions) error {
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

	vaults, err := resolveVaults(homeDir, lookup, opts.Vault)
	if err != nil {
		return writeVaultLocateError(out, opts.JSON, err)
	}

	matches, err := locateVaultMatches(vaults, opts.Filename, opts.Reference, opts.Query)
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
