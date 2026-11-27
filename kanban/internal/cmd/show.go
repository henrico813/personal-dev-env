package cmd

import (
	"fmt"
	"io"
	"strings"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// show --json's reply, a cardReply with the card's sections after it.
type showReply struct {
	cardReply
	Sections orderedObject `json:"sections"`
}

func newShowCommand(e *env) *cobra.Command {
	var section string
	var asJSON bool
	command := &cobra.Command{
		Use:     "show CARD_KEY",
		Short:   "print a card",
		Example: "  card show DEMO-001\n  card show DEMO-001 --section \"Open Questions\" --json",
		Args:    requireArgs("key"),
		RunE: func(command *cobra.Command, args []string) error {
			return runShow(e, command.OutOrStdout(), args[0], section, asJSON)
		},
	}
	command.Flags().StringVar(&section, "section", "", "any ## heading, in any case")
	addJSONFlag(command, &asJSON)
	return command
}

// Prints a whole card, or one section with each item numbered for card
// edit, and prints a section the card lacks as empty. JSON prints a
// cardReply and each section's text, or only the --section one.
func runShow(e *env, out io.Writer, cardKey, section string, asJSON bool) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	if section != "" {
		name, ok := card.FindSectionName(c.Text, section)
		if !ok {
			return fmt.Errorf("%s has no %s section", c.CardKey, section)
		}
		section = name
	}
	if asJSON {
		names := card.ListSectionNames(c.Text)
		if section != "" {
			names = []string{section}
		}
		reply := showReply{cardReply: describeCard(c), Sections: orderedObject{}}
		for _, name := range names {
			text, _ := card.ReadSection(c.Text, name)
			reply.Sections = append(reply.Sections, namedValue{name: name, value: strings.TrimSpace(text)})
		}
		return printJSON(out, reply)
	}
	shown := c.Text
	if section != "" {
		lines := strings.Join(card.ListNumberedSectionLines(c.Text, section), "\n")
		shown = "## " + section + "\n" + strings.TrimSpace(lines) + "\n"
	}
	_, err = io.WriteString(out, shown)
	return err
}
