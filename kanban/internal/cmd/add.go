package cmd

import (
	"fmt"
	"io"
	"time"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

func newAddCommand(e *env) *cobra.Command {
	return &cobra.Command{
		Use:     "add CARD_KEY SECTION TEXT",
		Short:   "add a bullet, or a criteria checkbox, to a section",
		Example: "  card add DEMO-001 \"Acceptance Criteria\" \"A failed upload retries\"\n  card add DEMO-001 \"Open Questions\" \"Retry at night?\"",
		Args: func(command *cobra.Command, args []string) error {
			if err := requireArgs("key", "section", "text")(command, args); err != nil {
				return err
			}
			return checkChoice("section", args[1], card.WritableSections)
		},
		RunE: func(command *cobra.Command, args []string) error {
			return runAdd(e, command.OutOrStdout(), args[0], args[1], args[2])
		},
	}
}

// Adds `- <text>` to a section, `- [ ] <text>` to Acceptance Criteria, or
// `- Q<n> asked <day>: <text>` to Open Questions, and prints the line.
func runAdd(e *env, out io.Writer, cardKey, section, text string) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	line := "- " + text
	if section == card.Criteria {
		line = "- [ ] " + text
	}
	_, err = card.Update(c, func(cardText string) (string, error) {
		if section == card.OpenQuestions {
			// Numbers older questions first, so the new one gets the next Q.
			cardText = card.StampQuestionNumbers(cardText)
			number := card.FindHighestQuestionNumber(cardText) + 1
			line = fmt.Sprintf("- Q%d asked %s: %s", number, e.today.Format(time.DateOnly), text)
		}
		return card.AddToSection(cardText, section, line), nil
	})
	if err != nil {
		return err
	}
	_, err = fmt.Fprintf(out, "%s %s: %s\n", c.CardKey, section, line)
	return err
}
