package cmd

import (
	"errors"
	"fmt"
	"io"
	"path/filepath"
	"strings"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// The options card new takes besides its arguments.
type newOptions struct {
	fromFile, kind, adopt, why, by string
	user                           bool
	session                        sessionFlag
}

func newNewCommand(e *env) *cobra.Command {
	var options newOptions
	command := &cobra.Command{
		Use:     "new PROJECT [TITLE]",
		Short:   "make a card in draft",
		Long:    formatCardFileHelp(),
		Example: "  card new DevEnv \"Retry failed uploads\"\n  card new DevEnv --from card.md",
		Args: func(command *cobra.Command, args []string) error {
			if err := requireArgsAndOptional("title", "project")(command, args); err != nil {
				return err
			}
			if err := checkChoice("--kind", options.kind, card.Kinds); err != nil {
				return err
			}
			return checkChoice("--adopt", options.adopt, card.AdoptStages)
		},
		RunE: func(command *cobra.Command, args []string) error {
			title := ""
			if len(args) == 2 {
				title = args[1]
			}
			return runNew(e, command.OutOrStdout(), args[0], title, len(args) == 2, options)
		},
	}
	flags := command.Flags()
	flags.StringVar(&options.fromFile, "from", "", "take the title, kind and sections from FILE, laid out below")
	flags.StringVar(&options.kind, "kind", "", formatKindHelp())
	flags.BoolVar(&options.user, "user", false, "the user asked for this card; agents pass it only when told")
	flags.StringVar(&options.adopt, "adopt", "", "start here, for work already under way; needs --user")
	flags.StringVar(&options.why, "why", "", "a reason for the Log line")
	addSessionFlag(command, &options.session, "with --adopt, ")
	addAuthorFlag(command, &options.by)
	return command
}

// Makes a card in draft, or in the --adopt stage, and prints its file name
// without .md. The checks run in the Python card's order, so a mistyped
// command gets the same first refusal.
func runNew(e *env, out io.Writer, project, title string, hasTitle bool, options newOptions) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	hasFrom := options.fromFile != ""
	switch {
	case hasTitle == hasFrom:
		return errors.New("give either a TITLE or --from FILE")
	case hasFrom && options.kind != "":
		return errors.New("with --from, FILE gives the kind")
	case options.adopt != "" && !options.user:
		return errors.New("only the user adopts work already under way. Pass --user only when the user said so.") //nolint:staticcheck // the Python card's words
	case options.session.given && options.adopt == "":
		return errors.New("--session goes with --adopt. A draft card gets its session when it moves to active.") //nolint:staticcheck // the Python card's words
	}
	if err := card.RequireBlockedReason(options.adopt, options.why); err != nil {
		return err
	}
	newCard := card.NewCard{Title: title, Kind: options.kind}
	if newCard.Kind == "" {
		newCard.Kind = card.Kinds[0]
	}
	if hasFrom {
		if newCard, err = card.ReadNewCardFile(options.fromFile); err != nil {
			return err
		}
	}
	if err := card.RequirePassingTitle(projects.Rules, strings.TrimSpace(newCard.Title)); err != nil {
		return err
	}
	stage, verb := card.Draft, "by"
	if options.adopt != "" {
		stage, verb = options.adopt, "adopted by"
	}
	author, err := findAuthor(e, options.by)
	if err != nil {
		return err
	}
	logText := card.FormatMoveLogText(stage, verb+" "+author, options.user, options.why)
	var session *card.AgentSession
	if options.session.given {
		found, err := findAgentSession(e, options.session.agent)
		if err != nil {
			return err
		}
		session = &found
	}
	path, err := createWithLock(e, projects, project, newCard, stage, logText, session)
	if err != nil {
		return err
	}
	_, err = fmt.Fprintln(out, strings.TrimSuffix(filepath.Base(path), filepath.Ext(path)))
	return err
}

// Holds new.lock while card.Create takes the next number and writes the
// note, so two card commands on one PC never take one card key.
func createWithLock(e *env, projects card.Projects, project string, newCard card.NewCard, stage, logText string, session *card.AgentSession) (string, error) {
	release, err := card.HoldLock(joinStatePath(e, "new.lock"))
	if err != nil {
		return "", err
	}
	defer release()
	return card.Create(projects, project, newCard, stage, e.today, logText, session)
}

// Returns --kind's help, such as "build ends in a change ...; default build".
func formatKindHelp() string {
	parts := make([]string, 0, len(card.Kinds))
	for _, kind := range card.Kinds {
		parts = append(parts, kind+" "+card.KindMeanings[kind])
	}
	return strings.Join(parts, "; ") + "; default " + card.Kinds[0]
}

// Returns card new's help on the layout of a --from file.
func formatCardFileHelp() string {
	return strings.Join([]string{
		"--from FILE holds the title and kind, then sections:",
		"  ---",
		"  title: Retry failed uploads",
		"  kind: build",
		"  ---",
		"",
		"  ## Context",
		"  - Uploads fail on a slow link.",
		"",
		"  ## Acceptance Criteria",
		"  - [ ] A failed upload retries.",
		"",
		"kind is optional, " + card.Kinds[0] + " by default. Each of these",
		"sections may appear once, in any letter case:",
		"  " + strings.Join(card.WritableSections, ", "),
		"Section text is copied as written, so criteria need - [ ].",
	}, "\n")
}
