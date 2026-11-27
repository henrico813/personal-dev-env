package cmd

import (
	"fmt"
	"io"
	"slices"
	"strings"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// How runList prints its cards, as text lines, tab lines or JSON.
type listFormat int

const (
	listText listFormat = iota
	listTabs
	listJSON
)

// Builds "card list" with --stage, --project, --kind, --tabs and --json.
// Its Args check returns a usageError for a stage or kind outside the
// tables.
func newListCommand(e *env) *cobra.Command {
	var filter card.Filter
	var tabs, asJSON bool
	command := &cobra.Command{
		Use:     "list",
		Short:   "one line per card",
		Example: "  card list --stage active\n  card list --tabs --project DevEnv",
		Args: func(command *cobra.Command, args []string) error {
			if err := requireArgs()(command, args); err != nil {
				return err
			}
			if tabs && asJSON {
				return usageError{msg: "argument --json: not allowed with argument --tabs"}
			}
			return checkFilter(filter)
		},
		RunE: func(command *cobra.Command, _ []string) error {
			format := listText
			if tabs {
				format = listTabs
			} else if asJSON {
				format = listJSON
			}
			return runList(e, command.OutOrStdout(), filter, format)
		},
	}
	addFilterFlags(command, &filter)
	command.Flags().BoolVar(&tabs, "tabs", false, "whole lines for scripts: key, stage, kind, project, title")
	addJSONFlag(command, &asJSON)
	return command
}

// Prints one line per card in stage order, or a tab line per card for
// scripts, then the card count. JSON prints a cardReply per card instead.
func runList(e *env, out io.Writer, filter card.Filter, format listFormat) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	all, err := card.LoadAll(projects)
	if err != nil {
		return err
	}
	cards := card.FilterCards(all, filter)
	slices.SortStableFunc(cards, func(a, b card.Card) int {
		if byStage := findStageRank(a.ReadStage()) - findStageRank(b.ReadStage()); byStage != 0 {
			return byStage
		}
		return strings.Compare(a.CardKey, b.CardKey)
	})
	if format == listJSON {
		replies := []cardReply{}
		for _, c := range cards {
			replies = append(replies, describeCard(c))
		}
		return printJSON(out, replies)
	}
	var text strings.Builder
	for _, c := range cards {
		if format == listTabs {
			fields := []string{c.CardKey, c.ReadStage(), c.ReadKind(), c.ReadProject(), c.ReadTitle()}
			fmt.Fprintln(&text, strings.Join(fields, "\t"))
			continue
		}
		line := fmt.Sprintf("%s %-9s %-8s %s", c.CardKey, c.ReadStage(), c.ReadKind(), c.ReadTitle())
		fmt.Fprintln(&text, fitLine(line, lineWidth))
	}
	fmt.Fprintf(&text, "%d cards\n", len(cards))
	_, err = io.WriteString(out, text.String())
	return err
}

// Returns the stage's place in board order, with unknown stages last.
func findStageRank(stage string) int {
	if rank := slices.Index(card.Stages, stage); rank >= 0 {
		return rank
	}
	return len(card.Stages)
}
