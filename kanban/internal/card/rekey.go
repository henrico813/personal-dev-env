package card

import (
	"context"
	"errors"
	"fmt"
	"io/fs"
	"maps"
	"os"
	"path/filepath"
	"regexp"
	"slices"
	"strconv"
	"strings"
	"time"
)

// Matches a wiki link's target, such as DEMO-001 in [[DEMO-001#Log|demo]].
var linkTarget = regexp.MustCompile(`\[\[([^\]|#\n]+)`)

// Matches a word shell quoting can leave bare, as Python's shlex.quote.
var shellSafe = regexp.MustCompile(`^[A-Za-z0-9_@%+=:,./-]+$`)

// A note's new text, and its new path when rekey renames it.
type NoteChange struct {
	Path, NewPath, NewText string
	ModTime                time.Time
}

// An open card's card key before and after a rekey, such as DEMO-KB-012
// to DEMO-523.
type KeyChange struct{ Old, New string }

// Gives each open card with an old card key the next free card key of its
// project code. Codes come in order, and each code's cards in the order of
// their old numbers.
func NumberOpenCards(projects Projects, cards []Card) ([]KeyChange, error) {
	keyRule, err := projects.Rules.FindNameRule("card_key")
	if err != nil {
		return nil, err
	}
	type numbered struct {
		n       int
		cardKey string
	}
	byCode := map[string][]numbered{}
	for _, c := range cards {
		if slices.Contains([]string{Done, Dropped}, c.ReadStage()) || !strings.Contains(c.CardKey, "-KB-") {
			continue
		}
		// card_file and card_key read one shape, so every card key parses.
		fields, ok := ParseName(keyRule, c.CardKey)
		if !ok {
			continue
		}
		n, err := strconv.Atoi(fields["n"])
		if err != nil {
			return nil, err
		}
		byCode[fields["code"]] = append(byCode[fields["code"]], numbered{n, c.CardKey})
	}
	changes := []KeyChange{}
	for _, code := range slices.Sorted(maps.Keys(byCode)) {
		first, err := FindNextNumber(projects, code)
		if err != nil {
			return nil, err
		}
		keys := byCode[code]
		slices.SortFunc(keys, func(a, b numbered) int {
			if a.n != b.n {
				return a.n - b.n
			}
			return strings.Compare(a.cardKey, b.cardKey)
		})
		for offset, key := range keys {
			values := map[string]string{"code": code, "n": strconv.Itoa(first + offset)}
			changes = append(changes, KeyChange{Old: key.cardKey, New: MakeName(keyRule, values)})
		}
	}
	return changes, nil
}

// Lists the cards a running agent works on, as "<card key> (<agent>)". An
// agent works on a card when its name holds the card key under the agent
// rule, or its session entry is in the card's sessions.
func ListCardsHeldByAgents(cards []Card, agents []AgentSession, rules NameRules) ([]string, error) {
	agentRule, err := rules.FindNameRule("agent")
	if err != nil {
		return nil, err
	}
	held := []string{}
	for _, agent := range agents {
		fields, named := ParseName(agentRule, agent.Name)
		shown := agent.Name
		if shown == "" {
			shown = agent.Entry
		}
		for _, c := range cards {
			byName := named && strings.ToUpper(fields["key"]) == c.CardKey
			sessions := ReadFields(c.Text)["sessions"].AsList()
			if byName || (agent.Entry != "" && slices.Contains(sessions, agent.Entry)) {
				held = append(held, fmt.Sprintf("%s (%s)", c.CardKey, shown))
			}
		}
	}
	return held, nil
}

// Lists each rekeyed card's new path, old_key field and heading, and writes
// nothing. Links change later, once every card has its new name.
func ListCardRenames(cards []Card, newKeys map[string]string) ([]NoteChange, error) {
	changes := []NoteChange{}
	for _, c := range cards {
		newKey, ok := newKeys[c.CardKey]
		if !ok {
			continue
		}
		name := filepath.Base(c.Path)
		newPath := filepath.Join(filepath.Dir(c.Path), newKey+name[len(c.CardKey):])
		if _, err := os.Stat(newPath); err == nil {
			return nil, fmt.Errorf("%s already exists", filepath.Base(newPath))
		}
		// Read again, as the Python card does, so a save made while rekey
		// waited on herdr is kept rather than refused.
		fresh, err := Load(c.Path, c.CardKey)
		if err != nil {
			return nil, err
		}
		text, err := SetField(fresh.Text, "old_key", FieldValue{Text: c.CardKey})
		if err != nil {
			return nil, err
		}
		text = SetTitle(text, newKey, c.ReadTitle())
		changes = append(changes, NoteChange{Path: c.Path, NewPath: newPath, NewText: text, ModTime: fresh.ModTime})
	}
	return changes, nil
}

// Lists each note under Projects whose links change, and writes nothing.
// Each link to a card's old file name, old_key or card key in newKeys
// follows the card's new name and keeps its #heading and |alias.
func ListLinkChanges(projects Projects, cards []Card, newKeys map[string]string) ([]NoteChange, error) {
	targets := map[string]string{}
	for _, c := range cards {
		stem := strings.TrimSuffix(filepath.Base(c.Path), ".md")
		title := stem[len(c.CardKey):]
		oldKey, ok := ReadFields(c.Text)["old_key"]
		if ok && !oldKey.IsList && !slices.Contains([]string{Done, Dropped}, c.ReadStage()) {
			targets[oldKey.Text+title] = stem
			targets[oldKey.Text] = c.CardKey
		}
		if newKey, ok := newKeys[c.CardKey]; ok {
			targets[stem] = newKey + title
			targets[c.CardKey] = newKey
		}
	}
	changes := []NoteChange{}
	// WalkDir visits names in sorted order, as Python's sorted rglob does.
	err := filepath.WalkDir(projects.Dir, func(path string, entry fs.DirEntry, err error) error {
		if err != nil || entry.IsDir() || !strings.HasSuffix(path, ".md") {
			return err
		}
		info, err := os.Stat(path)
		if err != nil {
			return err
		}
		data, err := os.ReadFile(path)
		if err != nil {
			return err
		}
		text := normalizeNewlines(string(data))
		newText := linkTarget.ReplaceAllStringFunc(text, func(link string) string {
			if target, ok := targets[link[2:]]; ok {
				return "[[" + target
			}
			return link
		})
		if newText != text {
			changes = append(changes, NoteChange{Path: path, NewPath: path, NewText: newText, ModTime: info.ModTime()})
		}
		return nil
	})
	return changes, err
}

// Backs up, writes and renames each note in order, and returns the notes
// it wrote. It stops before the next note when ctx ends, or at the first
// note that fails, such as one edited since the listing.
func WriteRekeyChanges(ctx context.Context, projects Projects, backup string, changes []NoteChange) ([]NoteChange, error) {
	written := []NoteChange{}
	for _, change := range changes {
		name := filepath.Base(change.Path)
		stop := fmt.Errorf("stopped at %s; run card rekey --apply --user again to finish, or the undo line", name)
		if ctx.Err() != nil {
			return written, stop
		}
		wrote, err := backUpAndWrite(projects, backup, change)
		if err != nil {
			return written, errors.Join(err, stop)
		}
		if !wrote {
			return written, stop
		}
		written = append(written, change)
		if change.NewPath == change.Path {
			continue
		}
		if err := os.Rename(change.Path, change.NewPath); err != nil {
			// The card has old_key now, so a second run refuses until undo.
			return written, errors.Join(err, fmt.Errorf("stopped at %s; run the undo line, then card rekey --apply --user again", name))
		}
	}
	return written, nil
}

// Writes undo.sh in the backup folder and returns its path. It restores
// the written notes newest first, since a renamed card is written again
// when its links change, and removes each renamed card's new file.
func WriteUndoScript(projects Projects, backup string, written []NoteChange) (string, error) {
	lines := []string{"#!/bin/sh", "set -e"}
	for _, change := range slices.Backward(written) {
		if change.NewPath != change.Path {
			lines = append(lines, "rm -f -- "+QuoteShell(change.NewPath))
		}
		copied, err := findBackupPath(projects, backup, change.Path)
		if err != nil {
			return "", err
		}
		lines = append(lines, "cp -p -- "+QuoteShell(copied)+" "+QuoteShell(change.Path))
	}
	undo := filepath.Join(backup, "undo.sh")
	return undo, os.WriteFile(undo, []byte(strings.Join(lines, "\n")+"\n"), 0o666)
}

// Quotes a word for sh as Python's shlex.quote does, such as 'a b' for
// "a b", and leaves a safe word such as /tmp/x.md bare.
func QuoteShell(word string) string {
	if shellSafe.MatchString(word) {
		return word
	}
	return "'" + strings.ReplaceAll(word, "'", `'"'"'`) + "'"
}

// Copies the note into the backup, keeping its path under Projects, its
// mode and its modified time, then writes the new text if the note's time
// is still the one the listing read.
func backUpAndWrite(projects Projects, backup string, change NoteChange) (bool, error) {
	// Note names such as AGENTS.md repeat across project folders.
	copied, err := findBackupPath(projects, backup, change.Path)
	if err != nil {
		return false, err
	}
	if err := os.MkdirAll(filepath.Dir(copied), 0o777); err != nil {
		return false, err
	}
	info, err := os.Stat(change.Path)
	if err != nil {
		return false, err
	}
	data, err := os.ReadFile(change.Path)
	if err != nil {
		return false, err
	}
	if err := os.WriteFile(copied, data, info.Mode().Perm()); err != nil {
		return false, err
	}
	if err := os.Chtimes(copied, info.ModTime(), info.ModTime()); err != nil {
		return false, err
	}
	stem := strings.TrimSuffix(filepath.Base(change.Path), ".md")
	return writeSafely(change.Path, stem, change.NewText, change.ModTime)
}

// Returns where the backup keeps a note, such as <backup>/Demo/Links.md.
func findBackupPath(projects Projects, backup, path string) (string, error) {
	relative, err := filepath.Rel(projects.Dir, path)
	if err != nil {
		return "", err
	}
	return filepath.Join(backup, relative), nil
}
