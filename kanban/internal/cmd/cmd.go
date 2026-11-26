// Package cmd turns card's command line into calls to internal/card and
// prints the results. Read Main first, then one command's file.
package cmd

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"slices"
	"strings"
	"time"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

// The names.toml path card reads when PDE_NAMES_FILE is unset, written as
// its error message shows it.
const defaultNamesFile = "~/.config/pde/names.toml"

// What Main reads from the environment once. Commands get every outside
// input as a value.
type env struct {
	today     time.Time // CARD_TODAY, else the local date, at midnight UTC
	home      string    // HOME, for ~/.config/pde
	namesFile string    // PDE_NAMES_FILE, else defaultNamesFile
	vault     string    // the global --vault, empty for the default vault
}

// A mistyped command, such as a stage outside the allowed list. Main exits
// 2 for it, as argparse does.
type usageError struct{ msg string }

func (e usageError) Error() string { return e.msg }

// The command already printed its result, such as no grep match or check
// problems. Main exits 1 and prints nothing more.
var errExitOne = errors.New("exit 1")

// Runs one card command and returns its exit code, which is 0, 1 for a card
// error or a printed failure such as no grep match, or 2 for a usage error.
func Main(args []string) int {
	root := newRootCommand(&env{})
	root.SetArgs(args[1:])
	ran, err := root.ExecuteC()
	var usage usageError
	switch {
	case err == nil:
		return 0
	case errors.Is(err, errExitOne):
		return 1
	case errors.As(err, &usage):
		fmt.Fprintf(os.Stderr, "usage: %s\n%s: error: %s\n", ran.UseLine(), ran.CommandPath(), usage.msg)
		return 2
	}
	fmt.Fprintf(os.Stderr, "card: %s\n", err)
	return 1
}

// Reads CARD_TODAY, HOME and PDE_NAMES_FILE. A bad CARD_TODAY is a card
// error, so a test never runs on the real date by mistake.
func readEnv() (env, error) {
	e := env{home: os.Getenv("HOME"), namesFile: os.Getenv("PDE_NAMES_FILE")}
	if e.namesFile == "" {
		e.namesFile = defaultNamesFile
	}
	shown := os.Getenv("CARD_TODAY")
	if shown == "" {
		now := time.Now()
		e.today = time.Date(now.Year(), now.Month(), now.Day(), 0, 0, 0, 0, time.UTC)
		return e, nil
	}
	today, err := time.Parse(time.DateOnly, shown)
	if err != nil {
		return env{}, fmt.Errorf("CARD_TODAY must be a YYYY-MM-DD date, not %s", shown)
	}
	e.today = today
	return e, nil
}

// Builds the cobra command tree, one subcommand per file in this package,
// each with an Example. The environment is read after the arguments parse,
// so a usage error exits 2 whatever CARD_TODAY holds.
func newRootCommand(e *env) *cobra.Command {
	root := &cobra.Command{
		Use:     "card",
		Short:   "Keep kanban cards as notes in the Obsidian vault.",
		Example: "  card board\n  card show DEMO-001 --section Context",
		// The root runs no command of its own. Without these two, cobra
		// prints help for "card" alone and an unknown command exits 1.
		Args: func(_ *cobra.Command, args []string) error {
			if len(args) == 0 {
				return usageError{msg: "the following arguments are required: command"}
			}
			return usageError{msg: fmt.Sprintf("invalid choice: %q", args[0])}
		},
		Run:           func(*cobra.Command, []string) {},
		SilenceErrors: true,
		SilenceUsage:  true,
		PersistentPreRunE: func(*cobra.Command, []string) error {
			read, err := readEnv()
			if err != nil {
				return err
			}
			read.vault = e.vault
			*e = read
			return nil
		},
	}
	root.CompletionOptions.DisableDefaultCmd = true
	root.SetHelpCommand(&cobra.Command{Hidden: true})
	root.SetFlagErrorFunc(func(_ *cobra.Command, err error) error {
		return usageError{msg: err.Error()}
	})
	root.PersistentFlags().StringVar(&e.vault, "vault", "", "vault name for nobs vault path")
	root.AddCommand(
		newListCommand(e),
		newBoardCommand(e),
		newShowCommand(e),
		newOutlineCommand(e),
		newGrepCommand(e),
		newCheckCommand(e),
		newNameCommand(e),
		newTemplateCommand(e),
	)
	return root
}

// Checks that a command got exactly these positional arguments, so a
// missing or extra one exits 2, as with argparse.
func requireArgs(names ...string) cobra.PositionalArgs {
	return func(_ *cobra.Command, args []string) error {
		switch {
		case len(args) < len(names):
			return usageError{msg: "the following arguments are required: " + strings.Join(names[len(args):], ", ")}
		case len(args) > len(names):
			return usageError{msg: "unrecognized arguments: " + strings.Join(args[len(names):], " ")}
		}
		return nil
	}
}

// Fails with a usage error when an option is set to a value outside
// choices, as argparse's choices do.
func checkChoice(option, value string, choices []string) error {
	if value == "" || slices.Contains(choices, value) {
		return nil
	}
	return usageError{msg: fmt.Sprintf("argument %s: invalid choice: %q (choose from %s)", option, value, strings.Join(choices, ", "))}
}

// Adds --stage, --project and --kind, which list and grep share.
func addFilterFlags(command *cobra.Command, filter *card.Filter) {
	flags := command.Flags()
	flags.StringVar(&filter.Stage, "stage", "", "one of "+strings.Join(card.Stages, ", "))
	flags.StringVar(&filter.Project, "project", "", "project folder, any case")
	flags.StringVar(&filter.Kind, "kind", "", "one of "+strings.Join(card.Kinds, ", "))
}

// Fails with a usage error for a stage or kind outside the tables.
func checkFilter(filter card.Filter) error {
	if err := checkChoice("--stage", filter.Stage, card.Stages); err != nil {
		return err
	}
	return checkChoice("--kind", filter.Kind, card.Kinds)
}

// Adds --json, for commands that scripts read.
func addJSONFlag(command *cobra.Command, asJSON *bool) {
	command.Flags().BoolVar(asJSON, "json", false, "print JSON for scripts, with whole titles")
}

// Reads names.toml from e.namesFile, after a leading ~ becomes HOME. A
// missing file fails with the path as shown, such as ~/.config/pde/names.toml.
func loadNameRules(e *env) (card.NameRules, error) {
	path := e.namesFile
	if rest, ok := strings.CutPrefix(path, "~/"); ok {
		path = filepath.Join(e.home, rest)
	}
	if info, err := os.Stat(path); err != nil || !info.Mode().IsRegular() {
		return card.NameRules{}, fmt.Errorf("no naming rules at %s: run pde-installer install", e.namesFile)
	}
	return card.ReadNameRules(path)
}

// Finds the Projects folder through nobs and loads the naming rules. Every
// command that reads cards starts here.
func openProjects(e *env) (card.Projects, error) {
	dir, err := askNobsForProjectsFolder(e.vault)
	if err != nil {
		return card.Projects{}, err
	}
	rules, err := loadNameRules(e)
	if err != nil {
		return card.Projects{}, err
	}
	return card.Projects{Dir: dir, Rules: rules}, nil
}
