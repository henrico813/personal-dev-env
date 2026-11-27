package cmd

import (
	"fmt"
	"io"
	"regexp"
	"slices"
	"strings"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// grep --json's reply, with whole lines.
type grepReply struct {
	Matches    []matchReply `json:"matches"`
	MatchCount int          `json:"match_count"`
	CardCount  int          `json:"card_count"`
}

// One matching line of grep --json.
type matchReply struct {
	Key     string `json:"key"`
	Section string `json:"section"`
	Line    string `json:"line"`
}

func newGrepCommand(e *env) *cobra.Command {
	var filter card.Filter
	var section string
	var asJSON bool
	command := &cobra.Command{
		Use:     "grep PATTERN",
		Short:   "search card text",
		Example: "  card grep 'retry|timeout' --stage active\n  card grep bucket --section Decisions --json",
		Args: func(command *cobra.Command, args []string) error {
			if err := requireArgs("pattern")(command, args); err != nil {
				return err
			}
			if err := checkChoice("--section", section, card.Sections); err != nil {
				return err
			}
			return checkFilter(filter)
		},
		RunE: func(command *cobra.Command, args []string) error {
			return runGrep(e, command.OutOrStdout(), args[0], section, filter, asJSON)
		},
	}
	command.Flags().StringVar(&section, "section", "", "one of "+strings.Join(card.Sections, ", "))
	addFilterFlags(command, &filter)
	addJSONFlag(command, &asJSON)
	return command
}

// Prints each matching line as `CARD_KEY Section: line`, cut to 60
// characters, then the count. It exits 1 with nothing on stderr when no
// line matched, which the hub reads as no matches.
func runGrep(e *env, out io.Writer, pattern, section string, filter card.Filter, asJSON bool) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	compiled, err := regexp.Compile("(?i)" + pattern)
	if err != nil {
		return fmt.Errorf("bad pattern: %w", err)
	}
	all, err := card.LoadAll(projects)
	if err != nil {
		return err
	}
	cards := card.FilterCards(all, filter)
	slices.SortStableFunc(cards, func(a, b card.Card) int {
		return strings.Compare(a.CardKey, b.CardKey)
	})
	matches := []card.Match{}
	matchedCards := map[string]bool{}
	for _, match := range card.FindMatches(cards, compiled) {
		if section == "" || match.Section == section {
			matches = append(matches, match)
			matchedCards[match.CardKey] = true
		}
	}
	if asJSON {
		reply := grepReply{Matches: []matchReply{}, MatchCount: len(matches), CardCount: len(matchedCards)}
		for _, match := range matches {
			reply.Matches = append(reply.Matches, matchReply{Key: match.CardKey, Section: match.Section, Line: match.Line})
		}
		err = printJSON(out, reply)
	} else {
		var text strings.Builder
		for _, match := range matches {
			line := match.CardKey + " " + match.Section + ": " + strings.TrimSpace(match.Line)
			fmt.Fprintln(&text, fitLine(line, lineWidth))
		}
		fmt.Fprintf(&text, "%d matches in %d cards\n", len(matches), len(matchedCards))
		_, err = io.WriteString(out, text.String())
	}
	if err != nil {
		return err
	}
	if len(matches) == 0 {
		return errExitOne
	}
	return nil
}
