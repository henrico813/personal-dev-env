package cmd

import (
	"fmt"
	"io"
	"os"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

func newAttachCommand(e *env) *cobra.Command {
	return &cobra.Command{
		Use:     "attach CARD_KEY FILE",
		Short:   "copy a file to attachments and embed it in Context",
		Example: "  card attach DEMO-001 screenshot.png",
		Args:    requireArgs("key", "file"),
		RunE: func(command *cobra.Command, args []string) error {
			return runAttach(e, command.OutOrStdout(), args[0], args[1])
		},
	}
}

// Copies file to the card's attachments folder, embeds it in Context and
// prints `<card key> Context: <line>`. A missing file is refused before
// the card is looked up, as in the Python card.
func runAttach(e *env, out io.Writer, cardKey, file string) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	if info, err := os.Stat(file); err != nil || !info.Mode().IsRegular() {
		return fmt.Errorf("no file at %s", file)
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	line, err := card.Attach(c, file)
	if err != nil {
		return err
	}
	_, err = fmt.Fprintf(out, "%s Context: %s\n", c.CardKey, line)
	return err
}
