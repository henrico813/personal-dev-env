package cmd

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"slices"
	"strings"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

func newSetCommand(e *env) *cobra.Command {
	return &cobra.Command{
		Use:     "set CARD_KEY FIELD VALUE",
		Short:   "change the title or kind",
		Example: "  card set DEMO-001 title \"Retry failed uploads\"\n  card set DEMO-001 kind research",
		Args: func(command *cobra.Command, args []string) error {
			if err := requireArgs("key", "field", "value")(command, args); err != nil {
				return err
			}
			return checkChoice("field", args[1], []string{"title", "kind"})
		},
		RunE: func(command *cobra.Command, args []string) error {
			if args[1] == "kind" {
				return runSetKind(e, command.OutOrStdout(), args[0], args[2])
			}
			return runSetTitle(e, command.OutOrStdout(), args[0], args[2])
		},
	}
}

// Sets the card's kind and prints "<card key> kind <kind>".
func runSetKind(e *env, out io.Writer, cardKey, kind string) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	if !slices.Contains(card.Kinds, kind) {
		return fmt.Errorf("kind is one of %s", strings.Join(card.Kinds, ", "))
	}
	_, err = card.Update(c, func(text string) (string, error) {
		return card.SetField(text, "kind", card.FieldValue{Text: kind})
	})
	if err != nil {
		return err
	}
	_, err = fmt.Fprintf(out, "%s kind %s\n", c.CardKey, kind)
	return err
}

// Writes the new title, then renames the note to match it, and prints the
// new file name without .md.
func runSetTitle(e *env, out io.Writer, cardKey, value string) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	title := strings.TrimSpace(value)
	if err := card.RequirePassingTitle(projects.Rules, title); err != nil {
		return err
	}
	fileName, err := card.MakeCardFileName(projects.Rules, c.CardKey, value)
	if err != nil {
		return err
	}
	newPath := filepath.Join(filepath.Dir(c.Path), fileName)
	if newPath != c.Path {
		if _, err := os.Stat(newPath); err == nil {
			return fmt.Errorf("%s already exists", fileName)
		}
	}
	_, err = card.Update(c, func(text string) (string, error) {
		return card.SetTitle(text, c.CardKey, title), nil
	})
	if err != nil {
		return err
	}
	if newPath != c.Path {
		if err := os.Rename(c.Path, newPath); err != nil {
			return err
		}
	}
	_, err = fmt.Fprintln(out, strings.TrimSuffix(fileName, filepath.Ext(fileName)))
	return err
}
