package cmd

import (
	"fmt"
	"io"
	"strings"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// The options card move takes besides its arguments.
type moveOptions struct {
	why, by string
	user    bool
	session sessionFlag
}

func newMoveCommand(e *env) *cobra.Command {
	var options moveOptions
	command := &cobra.Command{
		Use:     "move CARD_KEY STAGE",
		Short:   "move a card to another stage",
		Long:    formatMovesHelp(),
		Example: "  card move DEMO-001 active --session\n  card move DEMO-001 blocked --why \"waits on the user\"",
		Args: func(command *cobra.Command, args []string) error {
			if err := requireArgs("key", "stage")(command, args); err != nil {
				return err
			}
			return checkChoice("stage", args[1], card.Stages)
		},
		RunE: func(command *cobra.Command, args []string) error {
			return runMove(e, command.OutOrStdout(), args[0], args[1], options)
		},
	}
	flags := command.Flags()
	flags.BoolVar(&options.user, "user", false, "the user made this move; agents pass it only when told")
	flags.StringVar(&options.why, "why", "", "a reason for the Log line")
	addSessionFlag(command, &options.session, "with a move to active, ")
	addAuthorFlag(command, &options.by)
	return command
}

// Moves a card to target through the shortest chain of allowed moves, in
// one write with one Log line per move, and prints "<card key> <stage>"
// for each.
func runMove(e *env, out io.Writer, cardKey, target string, options moveOptions) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	if options.session.given && target != card.Active {
		return fmt.Errorf("--session goes with a move to %s", card.Active)
	}
	if err := card.RequireBlockedReason(target, options.why); err != nil {
		return err
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	author, err := findAuthor(e, options.by)
	if err != nil {
		return err
	}
	var session card.AgentSession
	if options.session.given {
		if session, err = findAgentSession(e, options.session.agent); err != nil {
			return err
		}
	}
	var chain []string
	_, err = card.Update(c, func(text string) (string, error) {
		// A retry after an Obsidian save checks the moves again.
		var err error
		if chain, err = card.PlanMoves(projects, c.CardKey, text, target, options.user); err != nil {
			return "", err
		}
		for _, stage := range chain {
			why := ""
			if stage == target {
				why = options.why
			}
			logText := card.FormatMoveLogText(stage, "by "+author, options.user, why)
			if text, err = card.ApplyMove(text, stage, e.today, logText); err != nil {
				return "", err
			}
		}
		if !options.session.given {
			return text, nil
		}
		return card.AddSession(text, e.today, session)
	})
	if err != nil {
		return err
	}
	for _, stage := range chain {
		if _, err := fmt.Fprintf(out, "%s %s\n", c.CardKey, stage); err != nil {
			return err
		}
	}
	return nil
}

// Returns card move's help, one allowed move per line, then the chain,
// limit and blocked rules.
func formatMovesHelp() string {
	lines := []string{"moves (user moves need --user; the user also makes staff moves, with --user):"}
	for _, move := range card.Moves {
		lines = append(lines, fmt.Sprintf("  %s -> %s: %s", move.From, move.To, move.Who))
	}
	last := len(card.ChainStages) - 1
	lines = append(lines,
		fmt.Sprintf("a stage no single move reaches takes the shortest chain of these moves, through %s and %s, one Log line each",
			strings.Join(card.ChainStages[:last], ", "), card.ChainStages[last]),
		fmt.Sprintf("the user moves a card past the limits of %d %s and %d %s with --user",
			card.ActiveLimit, card.Active, card.ReviewLimit, card.InReview),
		fmt.Sprintf("a move to %s needs --why, naming who the card waits on", card.Blocked),
		"work already under way: card new --adopt STAGE --user",
	)
	return strings.Join(lines, "\n")
}
