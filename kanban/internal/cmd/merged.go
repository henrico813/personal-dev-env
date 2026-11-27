package cmd

import (
	"fmt"
	"io"
	"slices"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

func newMergedCommand(e *env) *cobra.Command {
	return &cobra.Command{
		Use:     "merged URL",
		Short:   "move the cards linking a merged PR to done",
		Long:    "A card moves only when all its PRs merged and all its Acceptance Criteria are ticked. Otherwise it stays, and its Log says why; when gh fails, it stays unchanged.",
		Example: "  card merged https://github.com/me/repo/pull/7",
		Args:    requireArgs("url"),
		RunE: func(command *cobra.Command, args []string) error {
			return runMerged(e, command.OutOrStdout(), command.ErrOrStderr(), args[0])
		},
	}
}

// Moves each open card that links the merged PR to done as the user, since
// a merge is the user's approval, or logs why it stays. One card's failure
// prints `card: <card key> unchanged: <reason>` on stderr, and the other
// cards still go.
func runMerged(e *env, out, errOut io.Writer, url string) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	merged := card.NormalizePRURL(url)
	cards, err := card.LoadAll(projects)
	if err != nil {
		return err
	}
	var linking []card.Card
	for _, c := range cards {
		if c.ReadStage() != card.Done && c.ReadStage() != card.Dropped && slices.Contains(card.ListCardPRs(c.Text), merged) {
			linking = append(linking, c)
		}
	}
	if len(linking) == 0 {
		_, err := fmt.Fprintf(out, "no open card links %s\n", merged)
		return err
	}
	failed := false
	for _, c := range linking {
		line, err := card.ApplyMerge(projects, c, merged, readPRState, e.today)
		if err != nil {
			_, _ = fmt.Fprintf(errOut, "card: %s unchanged: %s\n", c.CardKey, err)
			failed = true
			continue
		}
		if _, err := fmt.Fprintln(out, line); err != nil {
			return err
		}
	}
	if failed {
		return errExitOne
	}
	return nil
}
