package cmd

import (
	"context"
	"errors"
	"fmt"
	"io"
	"os"
	"os/signal"
	"path/filepath"
	"slices"
	"strings"
	"syscall"

	"github.com/spf13/cobra"

	"kanban/internal/card"
)

func newRekeyCommand(e *env) *cobra.Command {
	var apply, user bool
	command := &cobra.Command{
		Use:     "rekey",
		Short:   "give open cards with old card keys new card keys",
		Example: "  card rekey\n  card rekey --apply --user",
		Args:    requireArgs(),
		RunE: func(command *cobra.Command, _ []string) error {
			return runRekey(e, command.OutOrStdout(), command.ErrOrStderr(), apply, user)
		},
	}
	flags := command.Flags()
	flags.BoolVar(&apply, "apply", false, "make the changes; without it, only print them")
	flags.BoolVar(&user, "user", false, "the user asked for this; needed with --apply")
	return command
}

// Prints each open card's old and new card key and each note whose links
// change, and with --apply backs up and writes them. It holds new.lock, so
// card new takes no number while the new card keys are handed out.
func runRekey(e *env, out, errOut io.Writer, apply, user bool) error {
	projects, err := openProjects(e)
	if err != nil {
		return err
	}
	if apply && !user {
		return errors.New("--apply needs --user; the user runs it after review")
	}
	release, err := card.HoldLock(joinStatePath(e, "new.lock"))
	if err != nil {
		return err
	}
	defer release()
	cards, err := card.LoadAll(projects)
	if err != nil {
		return err
	}
	if err := checkRekeyStart(cards); err != nil {
		return err
	}
	keyChanges, err := card.NumberOpenCards(projects, cards)
	if err != nil {
		return err
	}
	newKeys := map[string]string{}
	for _, change := range keyChanges {
		newKeys[change.Old] = change.New
	}
	if err := refuseHeldCards(projects, cards, newKeys); err != nil {
		return err
	}
	renames, err := card.ListCardRenames(cards, newKeys)
	if err != nil {
		return err
	}
	relinks, err := card.ListLinkChanges(projects, cards, newKeys)
	if err != nil {
		return err
	}
	for _, change := range keyChanges {
		if _, err := fmt.Fprintf(out, "%s %s\n", change.Old, change.New); err != nil {
			return err
		}
	}
	for _, change := range relinks {
		shown, err := filepath.Rel(projects.Dir, change.Path)
		if err != nil {
			return err
		}
		if _, err := fmt.Fprintf(out, "relink %s\n", shown); err != nil {
			return err
		}
	}
	if !apply || len(renames)+len(relinks) == 0 {
		_, err := fmt.Fprintf(out, "%d cards to rekey, %d notes to relink\n", len(renames), len(relinks))
		return err
	}
	return applyRekey(projects, out, errOut, filepath.Dir(joinStatePath(e, "new.lock")), renames)
}

// Refuses a card written but not renamed by a stopped rekey, and a card
// key that two cards hold as card key or old_key.
func checkRekeyStart(cards []card.Card) error {
	var stuck []string
	counts := map[string]int{}
	for _, c := range cards {
		counts[c.CardKey]++
		old, ok := card.ReadFields(c.Text)["old_key"]
		if !ok || old.IsList {
			continue
		}
		counts[old.Text]++
		if old.Text == c.CardKey {
			stuck = append(stuck, c.CardKey)
		}
	}
	if len(stuck) > 0 {
		return fmt.Errorf("%s written but not renamed; run the last undo line, then card rekey --apply --user again", strings.Join(stuck, ", "))
	}
	var shared []string
	for cardKey, count := range counts {
		if count > 1 {
			shared = append(shared, cardKey)
		}
	}
	if len(shared) > 0 {
		slices.Sort(shared)
		return fmt.Errorf("two cards hold %s as key or old_key", strings.Join(shared, ", "))
	}
	return nil
}

// Refuses the rekey when a running Herdr agent holds a card it would rekey.
func refuseHeldCards(projects card.Projects, cards []card.Card, newKeys map[string]string) error {
	agents, err := listLiveAgents()
	if err != nil {
		return err
	}
	var rekeyed []card.Card
	for _, c := range cards {
		if _, ok := newKeys[c.CardKey]; ok {
			rekeyed = append(rekeyed, c)
		}
	}
	held, err := card.ListCardsHeldByAgents(rekeyed, agents, projects.Rules)
	if err != nil {
		return err
	}
	if len(held) > 0 {
		return fmt.Errorf("live workers on %s; wait until they end", strings.Join(held, ", "))
	}
	return nil
}

// Writes the renames, then the links read again from the renamed cards,
// so a stop never leaves a link to a card key not yet given out. SIGINT or
// SIGTERM stops it before the next note, and undo.sh is written either way.
func applyRekey(projects card.Projects, out, errOut io.Writer, stateDir string, renames []card.NoteChange) error {
	backup, err := os.MkdirTemp(stateDir, "rekey-")
	if err != nil {
		return fmt.Errorf("%w, no note changed", err)
	}
	if _, err := fmt.Fprintf(out, "backup: %s\n", backup); err != nil {
		return err
	}
	ctx, stop := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer stop()
	written, err := card.WriteRekeyChanges(ctx, projects, backup, renames)
	var relinks []card.NoteChange
	if err == nil {
		relinks, err = listRelinksAfterRenames(projects)
	}
	if err == nil {
		var more []card.NoteChange
		more, err = card.WriteRekeyChanges(ctx, projects, backup, relinks)
		written = append(written, more...)
	}
	if err == nil {
		_, _ = fmt.Fprintf(out, "rekeyed %d cards, relinked %d notes\n", len(renames), len(relinks))
	} else {
		for _, line := range strings.Split(err.Error(), "\n") {
			_, _ = fmt.Fprintf(errOut, "card: %s\n", line)
		}
	}
	// undo.sh puts back only the written notes and keeps any edit to the
	// rest. Its line prints even when a print above failed.
	undo, undoErr := card.WriteUndoScript(projects, backup, written)
	if undoErr != nil {
		return undoErr
	}
	if _, printErr := fmt.Fprintf(out, "undo (newest first): sh %s\n", card.QuoteShell(undo)); printErr != nil {
		return printErr
	}
	if err != nil {
		return errExitOne
	}
	return nil
}

// Lists the link changes after every card has its new name, reading the
// cards again, so each link follows its card's name by old_key.
func listRelinksAfterRenames(projects card.Projects) ([]card.NoteChange, error) {
	cards, err := card.LoadAll(projects)
	if err != nil {
		return nil, err
	}
	return card.ListLinkChanges(projects, cards, nil)
}
