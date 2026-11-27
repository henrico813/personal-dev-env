package cmd

import (
	"fmt"
	"io"
	"strings"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// outline --json's reply.
type outlineReply struct {
	Key      string           `json:"key"`
	Headings []outlineHeading `json:"headings"`
}

// One heading of outline --json, with the line range the text form prints.
type outlineHeading struct {
	Heading string `json:"heading"`
	Level   int    `json:"level"`
	Start   int    `json:"start"`
	End     int    `json:"end"`
}

func newOutlineCommand(e *env) *cobra.Command {
	var asJSON bool
	command := &cobra.Command{
		Use:     "outline CARD_KEY",
		Short:   "list sections with their line numbers",
		Example: "  card outline DEMO-001\n  card outline DEMO-001 --json",
		Args:    requireArgs("key"),
		RunE: func(command *cobra.Command, args []string) error {
			return runOutline(e, command.OutOrStdout(), args[0], asJSON)
		},
	}
	addJSONFlag(command, &asJSON)
	return command
}

// Prints each "##" and "###" heading with the lines its part covers,
// counted from 1 as sed -n counts them, such as "21-26     ## Context".
func runOutline(e *env, out io.Writer, cardKey string, asJSON bool) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	c, err := card.Find(projects, cardKey)
	if err != nil {
		return err
	}
	ranges := card.ListHeadingRanges(c.Text)
	if asJSON {
		reply := outlineReply{Key: c.CardKey, Headings: []outlineHeading{}}
		for _, r := range ranges {
			reply.Headings = append(reply.Headings, outlineHeading{Heading: r.Heading, Level: r.Level, Start: r.Start, End: r.End})
		}
		return printJSON(out, reply)
	}
	var text strings.Builder
	for _, r := range ranges {
		span := fmt.Sprintf("%d-%d", r.Start, r.End)
		fmt.Fprintf(&text, "%-9s %s%s\n", span, strings.Repeat("  ", r.Level-2), r.Line)
	}
	_, err = io.WriteString(out, text.String())
	return err
}
