package cmd

import (
	"fmt"
	"io"
	"time"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

func newNoteCommand(e *env) *cobra.Command {
	var by string
	command := &cobra.Command{
		Use:     "note CARD_KEY TEXT",
		Short:   "add a note to the Log",
		Example: "  card note DEMO-001 \"plan approved\"",
		Args:    requireArgs("key", "text"),
		RunE: func(command *cobra.Command, args []string) error {
			return runNote(e, command.OutOrStdout(), args[0], args[1], by)
		},
	}
	addAuthorFlag(command, &by)
	return command
}

// Adds `- <day> note (<author>): <text>` to the Log and prints it.
func runNote(e *env, out io.Writer, cardKey, text, by string) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	author, err := findAuthor(e, by)
	if err != nil {
		return err
	}
	line := fmt.Sprintf("- %s note (%s): %s", e.today.Format(time.DateOnly), author, text)
	_, err = card.Update(c, func(text string) (string, error) {
		return card.AddToSection(text, "Log", line), nil
	})
	if err != nil {
		return err
	}
	_, err = fmt.Fprintf(out, "%s Log: %s\n", c.CardKey, line)
	return err
}
