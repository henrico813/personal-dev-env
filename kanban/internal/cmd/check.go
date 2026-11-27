package cmd

import (
	"fmt"
	"io"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// check --json's reply, with whole problem lines.
type checkReply struct {
	Cards    int      `json:"cards"`
	Problems []string `json:"problems"`
}

func newCheckCommand(e *env) *cobra.Command {
	var asJSON bool
	command := &cobra.Command{
		Use:     "check",
		Short:   "find broken cards",
		Example: "  card check\n  card check --json",
		Args:    requireArgs(),
		RunE: func(command *cobra.Command, _ []string) error {
			return runCheck(e, command.OutOrStdout(), asJSON)
		},
	}
	addJSONFlag(command, &asJSON)
	return command
}

// Prints each card whose layout or fields card cannot use, then the count,
// and exits 1 when it found a problem. It also lists card keys held twice,
// notes named like cards with no type field, and sessions on two cards.
func runCheck(e *env, out io.Writer, asJSON bool) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	cards, err := card.LoadAll(projects)
	if err != nil {
		return err
	}
	problems := []string{}
	seen := map[string]string{}
	for _, c := range cards {
		for _, issue := range card.FindIssues(projects.Rules, c) {
			problems = append(problems, c.CardKey+" "+issue)
		}
		if file, ok := seen[c.CardKey]; ok {
			problems = append(problems, c.CardKey+" also in "+file)
		}
		seen[c.CardKey] = filepath.Base(c.Path)
	}
	untyped, err := card.ListUntypedCardNotes(projects, cards)
	if err != nil {
		return err
	}
	for _, name := range untyped {
		problems = append(problems, name+" has a card name but no type: card")
	}
	// The hub maps a session to one card, so a session on two cards shows
	// one worker's state on the wrong card.
	holders := map[string]string{}
	for _, c := range cards {
		for _, entry := range card.ReadFields(c.Text)["sessions"].AsList() {
			holder, ok := holders[entry]
			if !ok {
				holders[entry] = c.CardKey
			} else if holder != c.CardKey {
				problems = append(problems, c.CardKey+" session also on "+holder+": "+entry)
			}
		}
	}
	if asJSON {
		err = printJSON(out, checkReply{Cards: len(cards), Problems: problems})
	} else {
		var text strings.Builder
		for _, problem := range problems {
			fmt.Fprintln(&text, fitLine(problem, lineWidth))
		}
		fmt.Fprintf(&text, "%d problems in %d cards\n", len(problems), len(cards))
		_, err = io.WriteString(out, text.String())
	}
	if err != nil {
		return err
	}
	if len(problems) > 0 {
		return errExitOne
	}
	return nil
}
