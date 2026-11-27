package cmd

import (
	"fmt"
	"io"
	"slices"
	"strings"
	"time"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// The board's JSON reply, with the counts, the limits and both waiting
// lists.
type boardReply struct {
	Counts   orderedObject  `json:"counts"`
	Limits   boardLimits    `json:"limits"`
	InReview []cardReply    `json:"in-review"`
	Blocked  []blockedReply `json:"blocked"`
}

// The most cards active and in-review may hold.
type boardLimits struct {
	Active   int `json:"active"`
	InReview int `json:"in-review"`
}

// A blocked card in the board's JSON, with its days in blocked and the
// reason its newest move to blocked gave.
type blockedReply struct {
	cardReply
	Days   int    `json:"days"`
	Reason string `json:"reason"`
}

// A blocked card and its whole days in blocked.
type blockedCard struct {
	card card.Card
	days int
}

func newBoardCommand(e *env) *cobra.Command {
	var asJSON bool
	command := &cobra.Command{
		Use:     "board",
		Short:   "counts and review queue",
		Example: "  card board\n  card board --json",
		Args:    requireArgs(),
		RunE: func(command *cobra.Command, _ []string) error {
			return runBoard(e, command.OutOrStdout(), asJSON)
		},
	}
	addJSONFlag(command, &asJSON)
	return command
}

// Prints the count per stage, the limits, then the in-review and blocked
// cards, each list with the card that waited longest first. The hub reads
// the limits line, the blocked line and the BLOCKED list, so their words
// stay as they are.
func runBoard(e *env, out io.Writer, asJSON bool) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	cards, err := card.LoadAll(projects)
	if err != nil {
		return err
	}
	counts := map[string]int{}
	// A stage outside Stages, such as a typo, gets its own count after them.
	stages := slices.Clone(card.Stages)
	blocked := []blockedCard{}
	inReview := []card.Card{}
	for _, c := range cards {
		stage := c.ReadStage()
		if !slices.Contains(stages, stage) {
			stages = append(stages, stage)
		}
		counts[stage]++
		switch stage {
		case card.Blocked:
			blocked = append(blocked, blockedCard{card: c, days: countDaysSince(c.ReadStageDate(), e.today)})
		case card.InReview:
			inReview = append(inReview, c)
		}
	}
	slices.SortStableFunc(blocked, func(a, b blockedCard) int {
		if a.days != b.days {
			return b.days - a.days
		}
		return strings.Compare(a.card.CardKey, b.card.CardKey)
	})
	slices.SortStableFunc(inReview, func(a, b card.Card) int {
		if byDate := strings.Compare(a.ReadStageDate(), b.ReadStageDate()); byDate != 0 {
			return byDate
		}
		return strings.Compare(a.CardKey, b.CardKey)
	})
	if asJSON {
		reply := boardReply{
			Limits:   boardLimits{Active: card.ActiveLimit, InReview: card.ReviewLimit},
			InReview: []cardReply{},
			Blocked:  []blockedReply{},
		}
		for _, stage := range stages {
			reply.Counts = append(reply.Counts, namedValue{name: stage, value: counts[stage]})
		}
		for _, c := range inReview {
			reply.InReview = append(reply.InReview, describeCard(c))
		}
		for _, b := range blocked {
			reply.Blocked = append(reply.Blocked, blockedReply{
				cardReply: describeCard(b.card),
				Days:      b.days,
				Reason:    readBlockedReason(b.card.Text),
			})
		}
		return printJSON(out, reply)
	}
	var text strings.Builder
	// Three lines of stages keep each one within 40 columns.
	for _, row := range [][]string{card.Stages[:3], card.Stages[3:5], card.Stages[5:]} {
		shown := []string{}
		for _, stage := range row {
			shown = append(shown, fmt.Sprintf("%s %d", stage, counts[stage]))
		}
		fmt.Fprintln(&text, strings.Join(shown, "  "))
	}
	fmt.Fprintf(&text, "%s %d of %d, %s %d of %d\n",
		card.Active, counts[card.Active], card.ActiveLimit,
		card.InReview, counts[card.InReview], card.ReviewLimit)
	if len(blocked) > 0 {
		fmt.Fprintf(&text, "%s %d, oldest %s\n", card.Blocked, len(blocked), formatDays(blocked[0].days))
	}
	fmt.Fprintf(&text, "\n%s, oldest first (%d)\n", strings.ToUpper(card.InReview), len(inReview))
	for _, c := range inReview {
		fmt.Fprintln(&text, fitLine("- "+c.CardKey+" "+c.ReadStageDate(), boardWidth))
		fmt.Fprintln(&text, fitLine("  "+c.ReadTitle(), boardWidth))
	}
	if len(blocked) > 0 {
		fmt.Fprintf(&text, "\n%s, oldest first (%d)\n", strings.ToUpper(card.Blocked), len(blocked))
	}
	for _, b := range blocked {
		fmt.Fprintln(&text, fitLine("- "+b.card.CardKey+" "+formatDays(b.days), boardWidth))
		fmt.Fprintln(&text, fitLine("  "+b.card.ReadTitle(), boardWidth))
		if reason := readBlockedReason(b.card.Text); reason != "" {
			fmt.Fprintln(&text, fitLine("  "+reason, boardWidth))
		}
	}
	_, err = io.WriteString(out, text.String())
	return err
}

// Returns whole days from a date such as 2026-11-05 to today, never below
// 0. A date that does not parse counts as 0, so one bad card does not stop
// the board.
func countDaysSince(day string, today time.Time) int {
	since, err := time.Parse(time.DateOnly, day)
	if err != nil {
		return 0
	}
	return max(int(today.Sub(since).Hours()/24), 0)
}

// Returns "1 day" or "N days".
func formatDays(days int) string {
	if days == 1 {
		return "1 day"
	}
	return fmt.Sprintf("%d days", days)
}

// Returns the reason of a card's newest move to blocked, or "" when no Log
// line has one.
func readBlockedReason(text string) string {
	log, _ := card.ReadSection(text, "Log")
	reason := ""
	for _, line := range strings.Split(log, "\n") {
		words := strings.SplitN(line, " ", 3)
		if len(words) < 3 || words[0] != "-" {
			continue
		}
		if entry, ok := strings.CutPrefix(words[2], card.Blocked+": "); ok {
			_, after, _ := strings.Cut(entry, ". ")
			reason = strings.TrimSpace(after)
		}
	}
	return reason
}
