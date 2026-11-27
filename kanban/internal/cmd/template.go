package cmd

import (
	"io"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// Builds "card template", which prints the Obsidian template and needs no
// vault.
func newTemplateCommand(e *env) *cobra.Command {
	return &cobra.Command{
		Use:     "template",
		Short:   "print the Obsidian template",
		Example: "  card template > \"Templates/Kanban card.md\"",
		Args:    requireArgs(),
		RunE: func(command *cobra.Command, _ []string) error {
			return runTemplate(e, command.OutOrStdout())
		},
	}
}

// Prints the Templater script that makes a card from Obsidian, with the
// card_key, card_file and plan_file rules from names.toml.
func runTemplate(e *env, out io.Writer) error {
	rules, err := loadNameRules(e)
	if err != nil {
		return err
	}
	text, err := card.RenderTemplate(rules)
	if err != nil {
		return err
	}
	_, err = io.WriteString(out, text)
	return err
}
