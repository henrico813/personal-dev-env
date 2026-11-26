package cmd

import (
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

func newDeleteCommand(e *env) *cobra.Command {
	var user bool
	command := &cobra.Command{
		Use:     "delete CARD_KEY --user",
		Short:   "remove a card note",
		Example: "  card delete DEMO-001 --user",
		Args: func(command *cobra.Command, args []string) error {
			if err := requireArgs("key")(command, args); err != nil {
				return err
			}
			// Keeps argparse's message for a missing --user, which
			// MarkFlagRequired words differently.
			if !user {
				return usageError{msg: "the following arguments are required: --user"}
			}
			return nil
		},
		RunE: func(command *cobra.Command, args []string) error {
			return runDelete(e, command.OutOrStdout(), args[0])
		},
	}
	command.Flags().BoolVar(&user, "user", false, "")
	return command
}

// Removes the card's note and prints "deleted <file name>".
func runDelete(e *env, out io.Writer, cardKey string) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	if err := os.Remove(c.Path); err != nil {
		return err
	}
	_, err = fmt.Fprintf(out, "deleted %s\n", filepath.Base(c.Path))
	return err
}
