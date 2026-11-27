package cmd

import (
	"errors"
	"fmt"
	"io"
	"strings"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

func newLinkCommand(e *env) *cobra.Command {
	var session sessionFlag
	command := &cobra.Command{
		Use:     "link CARD_KEY [TARGET]",
		Short:   "link a plan note, a PR URL or a Herdr session",
		Example: "  card link DEMO-001 \"DEMO-012 Retry plan\"\n  card link DEMO-001 https://github.com/me/repo/pull/7\n  card link DEMO-001 --session",
		Args:    requireArgsAndOptional("target", "key"),
		RunE: func(command *cobra.Command, args []string) error {
			target := ""
			if len(args) == 2 {
				target = args[1]
			}
			return runLink(e, command.OutOrStdout(), args[0], target, len(args) == 2, session)
		},
	}
	addSessionFlag(command, &session, "")
	return command
}

// Links a plan, a PR URL or an agent's session to the card, and prints
// `<card key> <field>: <value>`, with " (already)" when the card had it.
func runLink(e *env, out io.Writer, cardKey, target string, hasTarget bool, session sessionFlag) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	if hasTarget == session.given {
		return errors.New("give a plan, a PR URL or --session")
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	if session.given {
		return linkSession(e, out, projects, c, session.agent)
	}
	return linkTarget(out, c, target)
}

// Adds a plan link, such as "[[DEMO-012 Retry plan]]", to plans, or a PR
// URL to prs.
func linkTarget(out io.Writer, c card.Card, target string) error {
	target = strings.TrimSpace(target)
	field, value := "prs", target
	if !strings.HasPrefix(target, "http://") && !strings.HasPrefix(target, "https://") {
		name := strings.TrimSuffix(strings.TrimPrefix(target, "[["), "]]")
		field, value = "plans", "[["+strings.TrimSuffix(name, ".md")+"]]"
	}
	written, err := card.Update(c, func(text string) (string, error) {
		return card.AddToList(text, field, value)
	})
	if err != nil {
		return err
	}
	return printLink(out, c.CardKey, field, value, written)
}

// Adds an agent's session to sessions, and logs it, unless another card
// holds that session; the hub maps each session to one card.
func linkSession(e *env, out io.Writer, projects card.Projects, c card.Card, agent string) error {
	session, err := findAgentSession(e, agent)
	if err != nil {
		return err
	}
	// The hub relinks a worker's own card when it parks the worker, so a
	// session this card holds passes even if another card has it.
	if _, held := card.FindSessionHolder([]card.Card{c}, session.Entry); !held {
		cards, err := card.LoadAll(projects)
		if err != nil {
			return err
		}
		if holder, found := card.FindSessionHolder(cards, session.Entry); found {
			return fmt.Errorf("session ID already on %s: %s. To log work on %s, run card note %s TEXT.", holder.CardKey, session.Entry, c.CardKey, c.CardKey) //nolint:staticcheck // the Python card's words
		}
	}
	written, err := card.Update(c, func(text string) (string, error) {
		return card.AddSession(text, e.today, session)
	})
	if err != nil {
		return err
	}
	return printLink(out, c.CardKey, "sessions", session.Entry, written)
}

// Prints `<card key> <field>: <value>`, with " (already)" when the card
// already had the value.
func printLink(out io.Writer, cardKey, field, value string, written bool) error {
	already := ""
	if !written {
		already = " (already)"
	}
	_, err := fmt.Fprintf(out, "%s %s: %s%s\n", cardKey, field, value, already)
	return err
}
