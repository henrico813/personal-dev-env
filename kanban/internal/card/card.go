package card

import (
	"bufio"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"time"
)

// One card note as last read from disk.
type Card struct {
	Path    string
	CardKey string    // the card key, such as DEMO-001
	Text    string    // the whole note, with "\r\n" read as "\n"
	ModTime time.Time // the note's modified time, taken before its text was read
}

// The vault's Projects folder, and the naming rules that say which notes in
// it are cards.
type Projects struct {
	Dir   string
	Rules NameRules
}

// Which cards a list keeps. An empty field keeps every card.
type Filter struct{ Stage, Project, Kind string }

// Reads the stage field, or "" when it is missing.
func (c Card) ReadStage() string {
	return ReadFields(c.Text)["stage"].Text
}

// Reads the kind field, or "" when it is missing.
func (c Card) ReadKind() string {
	return ReadFields(c.Text)["kind"].Text
}

// Reads the title from the first "# " heading after the frontmatter, else
// from the file name, without the card key. With no frontmatter the first
// line is skipped, as in the Python card.
func (c Card) ReadTitle() string {
	lines := strings.Split(c.Text, "\n")
	for _, line := range lines[findFrontmatterEnd(lines)+1:] {
		if heading, ok := strings.CutPrefix(line, "# "); ok {
			if title := trimCardKey(heading, c.CardKey); title != "" {
				return title
			}
			break
		}
	}
	return trimCardKey(strings.TrimSuffix(filepath.Base(c.Path), ".md"), c.CardKey)
}

// Reads stage_date, else date_created.
func (c Card) ReadStageDate() string {
	fields := ReadFields(c.Text)
	if date := fields["stage_date"].Text; date != "" {
		return date
	}
	return fields["date_created"].Text
}

// Reads the project folder's name from the path, such as DevEnv.
func (c Card) ReadProject() string {
	return filepath.Base(filepath.Dir(c.Path))
}

// Reads every card in every project folder, sorted by path. A note is a
// card only when its name holds a card key and its type field is card.
func LoadAll(projects Projects) ([]Card, error) {
	paths, err := filepath.Glob(filepath.Join(projects.Dir, "*", "*.md"))
	if err != nil {
		return nil, err
	}
	cards := []Card{}
	for _, path := range paths {
		found, ok, err := readCardNote(path, projects.Rules)
		if err != nil {
			return nil, err
		}
		if ok {
			cards = append(cards, found)
		}
	}
	return cards, nil
}

// Returns the one card with this card key, in any case, or the one that
// held it before a rekey. It fails with "no card <card key>".
func Find(projects Projects, cardKey string) (Card, error) {
	cardKey = strings.ToUpper(cardKey)
	paths, err := filepath.Glob(filepath.Join(projects.Dir, "*", cardKey+"*.md"))
	if err != nil {
		return Card{}, err
	}
	cards := []Card{}
	for _, path := range paths {
		if key, _ := ReadCardKey(projects.Rules, filepath.Base(path)); key != cardKey {
			continue
		}
		found, ok, err := readCardNote(path, projects.Rules)
		if err != nil {
			return Card{}, err
		}
		if ok {
			cards = append(cards, found)
		}
	}
	if len(cards) == 0 {
		// card rekey keeps a card's old card key in old_key.
		all, err := LoadAll(projects)
		if err != nil {
			return Card{}, err
		}
		for _, c := range all {
			if old := ReadFields(c.Text)["old_key"]; !old.IsList && old.Text == cardKey {
				cards = append(cards, c)
			}
		}
	}
	switch len(cards) {
	case 0:
		return Card{}, fmt.Errorf("no card %s", cardKey)
	case 1:
		return cards[0], nil
	}
	names := make([]string, len(cards))
	for i, c := range cards {
		names[i] = filepath.Base(c.Path)
	}
	return Card{}, fmt.Errorf("two files hold %s, fix by hand: %s", cardKey, strings.Join(names, ", "))
}

// Keeps the cards that match every set field of the filter. The project
// matches in any case.
func FilterCards(cards []Card, filter Filter) []Card {
	kept := []Card{}
	for _, c := range cards {
		if (filter.Stage == "" || c.ReadStage() == filter.Stage) &&
			(filter.Kind == "" || c.ReadKind() == filter.Kind) &&
			(filter.Project == "" || strings.EqualFold(c.ReadProject(), filter.Project)) {
			kept = append(kept, c)
		}
	}
	return kept
}

// Lists the file names of notes that are named like cards but are not
// cards, so every command skips them. A note counts when its name has
// -KB- or its frontmatter has a stage and no type.
func ListUntypedCardNotes(projects Projects, cards []Card) ([]string, error) {
	paths, err := filepath.Glob(filepath.Join(projects.Dir, "*", "*.md"))
	if err != nil {
		return nil, err
	}
	cardPaths := map[string]bool{}
	for _, c := range cards {
		cardPaths[c.Path] = true
	}
	names := []string{}
	for _, path := range paths {
		name := filepath.Base(path)
		if _, ok := ReadCardKey(projects.Rules, name); cardPaths[path] || !ok {
			continue
		}
		untyped := strings.Contains(name, "-KB-")
		if !untyped {
			if untyped, err = isCardWithoutType(path); err != nil {
				return nil, err
			}
		}
		if untyped {
			names = append(names, name)
		}
	}
	return names, nil
}

// Reports whether a note's frontmatter has a stage field but no type field.
func isCardWithoutType(path string) (bool, error) {
	note, err := os.Open(path)
	if err != nil {
		return false, err
	}
	// Closing a note opened only to read it loses no data.
	defer func() { _ = note.Close() }()
	head, err := readHead(bufio.NewReader(note))
	if err != nil {
		return false, err
	}
	fields := ReadFields(head)
	_, hasType := fields["type"]
	_, hasStage := fields["stage"]
	return !hasType && hasStage, nil
}

// Reads a note up to the end of its frontmatter, and the rest only when it
// is a card. A plan then costs one short read.
func readCardNote(path string, rules NameRules) (Card, bool, error) {
	cardKey, ok := ReadCardKey(rules, filepath.Base(path))
	if !ok {
		return Card{}, false, nil
	}
	info, err := os.Stat(path)
	if err != nil {
		return Card{}, false, err
	}
	note, err := os.Open(path)
	if err != nil {
		return Card{}, false, err
	}
	// Closing a note opened only to read it loses no data.
	defer func() { _ = note.Close() }()
	reader := bufio.NewReader(note)
	head, err := readHead(reader)
	if err != nil {
		return Card{}, false, err
	}
	if ReadFields(head)["type"].Text != "card" {
		return Card{}, false, nil
	}
	rest, err := io.ReadAll(reader)
	if err != nil {
		return Card{}, false, err
	}
	text := head + normalizeNewlines(string(rest))
	return Card{Path: path, CardKey: cardKey, Text: text, ModTime: info.ModTime()}, true, nil
}

// Reads a note's first line, and its frontmatter when it has one, leaving
// the reader after them.
func readHead(reader *bufio.Reader) (string, error) {
	var head strings.Builder
	line, err := readLine(reader)
	head.WriteString(line)
	if err != nil || strings.TrimSpace(line) != "---" {
		return head.String(), err
	}
	for {
		line, err := readLine(reader)
		head.WriteString(line)
		if err != nil || line == "" || strings.TrimSpace(line) == "---" {
			return head.String(), err
		}
	}
}

// Reads one line with its "\n", or "" at the end of the note.
func readLine(reader *bufio.Reader) (string, error) {
	line, err := reader.ReadString('\n')
	if errors.Is(err, io.EOF) {
		err = nil
	}
	return normalizeNewlines(line), err
}

// Turns "\r\n" and a lone "\r" into "\n", as Python's text reads do.
func normalizeNewlines(text string) string {
	return strings.ReplaceAll(strings.ReplaceAll(text, "\r\n", "\n"), "\r", "\n")
}

// Strips the card key from the front of a title, with the spaces around it.
func trimCardKey(title, cardKey string) string {
	return strings.TrimSpace(strings.TrimPrefix(strings.TrimSpace(title), cardKey))
}
