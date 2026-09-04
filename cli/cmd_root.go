package main

import "github.com/spf13/cobra"

func newRootCmd() *cobra.Command {
	root := &cobra.Command{
		Use:   "pde",
		Short: "Manage PDE vault tooling",
	}
	root.AddCommand(newVaultCmd())
	return root
}
