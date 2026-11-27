package cmd

import (
	"errors"
	"fmt"
	"io"
	"strings"
	"time"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// The options card edit takes besides its arguments. Exactly one of
// newText, remove, tick and untick says what changes.
type editOptions struct {
	newText              string
	hasText              bool
	remove, tick, untick bool
	by                   string
}

func newEditCommand(e *env) *cobra.Command {
	var options editOptions
	command := &cobra.Command{
		Use:     "edit CARD_KEY SECTION N [TEXT]",
		Short:   "replace, remove, tick or untick one item of a section",
		Long:    "N is the item's number in card show CARD_KEY --section SECTION, or Q<n> for the Open Questions item numbered Q<n>.",
		Example: "  card edit DEMO-001 \"Acceptance Criteria\" 2 --tick\n  card edit DEMO-001 \"Open Questions\" Q3 --remove",
		Args: func(command *cobra.Command, args []string) error {
			if err := requireArgsAndOptional("text", "key", "section", "number")(command, args); err != nil {
				return err
			}
			return checkChoice("section", args[1], card.WritableSections)
		},
		RunE: func(command *cobra.Command, args []string) error {
			if len(args) == 4 {
				options.newText, options.hasText = args[3], true
			}
			return runEdit(e, command.OutOrStdout(), args[0], args[1], args[2], options)
		},
	}
	flags := command.Flags()
	flags.BoolVar(&options.remove, "remove", false, "remove item N")
	flags.BoolVar(&options.tick, "tick", false, "set item N's checkbox to - [x]")
	flags.BoolVar(&options.untick, "untick", false, "set item N's checkbox to - [ ]")
	addAuthorFlag(command, &options.by)
	return command
}

// Replaces, removes, ticks or unticks item N, adds a Log line that keeps
// the old words, and prints that line. A replaced item keeps its bullet or
// checkbox, and in Open Questions its Q<n> head.
func runEdit(e *env, out io.Writer, cardKey, section, given string, options editOptions) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	changes := 0
	for _, chosen := range []bool{options.hasText, options.remove, options.tick, options.untick} {
		if chosen {
			changes++
		}
	}
	if changes != 1 {
		return errors.New("give the new text, --remove, --tick or --untick")
	}
	if options.hasText {
		if _, words := card.SplitBullet(strings.TrimSpace(options.newText)); words == "" {
			return errors.New("the new text is empty; use --remove")
		}
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	author, err := findAuthor(e, options.by)
	if err != nil {
		return err
	}
	item, err := card.FindItem(c.Text, c.CardKey, section, given)
	if err != nil {
		return err
	}
	label := section + " item " + given
	words := card.QuoteASCII(item.Words())
	var note string
	switch {
	case options.tick:
		note = "ticked " + label + ", " + words
	case options.untick:
		note = "unticked " + label + ", " + words
	case options.remove:
		note = "removed " + label + ", " + words
	default:
		note = "replaced " + label + ", was " + words
	}
	line := fmt.Sprintf("- %s note (%s): %s", e.today.Format(time.DateOnly), author, note)
	_, err = card.Update(c, func(text string) (string, error) {
		var err error
		switch {
		case options.tick || options.untick:
			text, err = card.TickItem(text, item, options.tick)
		case options.remove:
			text, err = card.RemoveItem(text, item)
		default:
			text, err = card.ReplaceItem(text, item, options.newText)
		}
		if err != nil {
			return "", err
		}
		return card.AddToSection(text, "Log", line), nil
	})
	if err != nil {
		return err
	}
	_, err = fmt.Fprintf(out, "%s Log: %s\n", c.CardKey, line)
	return err
}
