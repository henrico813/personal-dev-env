package card

import (
	"errors"
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"regexp"
	"slices"
	"strconv"
	"strings"
	"syscall"
	"time"
)

// The project code line in a project folder's AGENTS.md, such as
// `Project code: DEMO`, with or without backticks around the code.
var projectCodePattern = regexp.MustCompile("Project code:[" + spaceClass + "]*`?([A-Z][A-Z0-9]*)`?")

// What card new needs to write a card, from its arguments or a --from
// file. Sections maps a name from WritableSections to its text.
type NewCard struct {
	Title, Kind string
	Sections    map[string]string
}

// Reads a new card's title, kind and section text from a file laid out as
// card new --help shows. Text outside the writable sections is refused,
// so no line is dropped without notice.
func ReadNewCardFile(path string) (NewCard, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return NewCard{}, fmt.Errorf("cannot read %s: %s", path, describeOSError(err))
	}
	text := normalizeNewlines(string(data))
	fields := ReadFields(text)
	title := strings.TrimSpace(fields["title"].Text)
	if title == "" {
		return NewCard{}, fmt.Errorf("%s needs a title field in its frontmatter", path)
	}
	// A kind list, such as [build], is refused, as in the Python card.
	kind := fields["kind"].Text
	if kind == "" && len(fields["kind"].List) == 0 {
		kind = Kinds[0]
	}
	if !slices.Contains(Kinds, kind) {
		return NewCard{}, fmt.Errorf("kind is one of %s", strings.Join(Kinds, ", "))
	}
	seen := map[string]bool{}
	lines := strings.Split(text, "\n")
	for _, line := range lines[findFrontmatterEnd(lines)+1:] {
		heading := headingPattern.FindStringSubmatch(line)
		if heading == nil {
			if strings.TrimSpace(line) != "" && len(seen) == 0 {
				return NewCard{}, fmt.Errorf("%s has text before its first section: %s", path, quotePythonRepr(line))
			}
			continue
		}
		name := strings.ToLower(heading[2])
		allowed := slices.ContainsFunc(WritableSections, func(s string) bool { return strings.ToLower(s) == name })
		if heading[1] != "##" || !allowed || seen[name] {
			return NewCard{}, fmt.Errorf("%s has the heading %s. Each section may appear once: %s",
				path, quotePythonRepr(strings.TrimSpace(line)), strings.Join(WritableSections, ", "))
		}
		seen[name] = true
	}
	sections := map[string]string{}
	for _, name := range WritableSections {
		if body, ok := ReadSection(text, name); ok && strings.TrimSpace(body) != "" {
			sections[name] = strings.Trim(body, "\n")
		}
	}
	return NewCard{Title: title, Kind: kind, Sections: sections}, nil
}

// Returns one more than the highest card or plan number of a project code,
// in every folder, so a new card key never repeats an old card's number.
// The caller holds the lock from this call until the card is written.
func FindNextNumber(projects Projects, code string) (int, error) {
	cardFile, err := projects.Rules.FindNameRule("card_file")
	if err != nil {
		return 0, err
	}
	planFile, err := projects.Rules.FindNameRule("plan_file")
	if err != nil {
		return 0, err
	}
	highest := 0
	err = filepath.WalkDir(projects.Dir, func(path string, entry fs.DirEntry, err error) error {
		// Python's glob skips a folder it cannot read, so only the
		// Projects folder itself fails.
		if err != nil && path == projects.Dir {
			return err
		}
		if err != nil {
			return nil
		}
		name := entry.Name()
		if !strings.HasPrefix(name, code+"-") || !strings.HasSuffix(name, ".md") {
			return nil
		}
		fields, ok := ParseName(cardFile, name)
		if !ok {
			fields, ok = ParseName(planFile, name)
		}
		if !ok {
			return nil
		}
		// An old card key such as DEMO-KB-012 counts as 12.
		digits := strings.Map(func(r rune) rune {
			if r >= '0' && r <= '9' {
				return r
			}
			return -1
		}, fields["key"][len(code):])
		if number, err := strconv.Atoi(digits); err == nil {
			highest = max(highest, number)
		}
		return nil
	})
	return highest + 1, err
}

// Takes the next card key in project, writes a new card in stage with its
// first Log line and any session, and returns its path. The caller holds
// new.lock from HoldLock around the call.
func Create(projects Projects, project string, c NewCard, stage string, today time.Time, logText string, session *AgentSession) (string, error) {
	folder, err := findProjectFolder(projects.Dir, project)
	if err != nil {
		return "", err
	}
	code, err := readProjectCode(folder)
	if err != nil {
		return "", err
	}
	name := strings.TrimSpace(badNameChars.ReplaceAllString(c.Title, ""))
	if name == "" {
		return "", errors.New("the title needs a letter or digit")
	}
	keyRule, err := projects.Rules.FindNameRule("card_key")
	if err != nil {
		return "", err
	}
	fileRule, err := projects.Rules.FindNameRule("card_file")
	if err != nil {
		return "", err
	}
	number, err := FindNextNumber(projects, code)
	if err != nil {
		return "", err
	}
	cardKey := MakeName(keyRule, map[string]string{"code": code, "n": strconv.Itoa(number)})
	path := filepath.Join(folder, MakeName(fileRule, map[string]string{"key": cardKey, "title": name}))
	day := today.Format(time.DateOnly)
	text := renderCard(cardKey, strings.TrimSpace(c.Title), filepath.Base(folder), c.Kind, day, stage, logText, c.Sections)
	if session != nil {
		if text, err = AddSession(text, today, *session); err != nil {
			return "", err
		}
	}
	if _, err := writeSafely(path, cardKey, text, time.Time{}); err != nil {
		return "", err
	}
	// The lock covers only the PC that runs the command, so a card made in
	// Obsidian or on another PC at the same moment can still take the number.
	holders, err := listCardKeyHolders(projects.Rules, folder, cardKey)
	if err != nil {
		return "", err
	}
	if len(holders) > 1 {
		return "", fmt.Errorf("two files hold %s, fix by hand: %s", cardKey, strings.Join(holders, ", "))
	}
	return path, nil
}

// Finds a project folder by name, in any case.
func findProjectFolder(dir, project string) (string, error) {
	entries, err := os.ReadDir(dir)
	if err != nil {
		return "", err
	}
	for _, entry := range entries {
		if entry.IsDir() && strings.EqualFold(entry.Name(), project) {
			return filepath.Join(dir, entry.Name()), nil
		}
	}
	return "", fmt.Errorf("no project folder %s in %s", project, dir)
}

// Reads the project code, such as DEMO, from the folder's AGENTS.md.
func readProjectCode(folder string) (string, error) {
	data, _ := os.ReadFile(filepath.Join(folder, "AGENTS.md"))
	match := projectCodePattern.FindSubmatch(data)
	if match == nil {
		return "", fmt.Errorf("%s has no 'Project code:' line in AGENTS.md. Ask the user for one.", filepath.Base(folder)) //nolint:staticcheck // the Python card's words
	}
	return string(match[1]), nil
}

// Lists the names of the notes in folder that hold cardKey under the
// card_file rule.
func listCardKeyHolders(rules NameRules, folder, cardKey string) ([]string, error) {
	entries, err := os.ReadDir(folder)
	if err != nil {
		return nil, err
	}
	var holders []string
	for _, entry := range entries {
		name := entry.Name()
		if !strings.HasPrefix(name, cardKey) || !strings.HasSuffix(name, ".md") {
			continue
		}
		if key, ok := ReadCardKey(rules, name); ok && key == cardKey {
			holders = append(holders, name)
		}
	}
	return holders, nil
}

// Describes an error from the operating system in the Python card's
// words, such as "No such file or directory".
func describeOSError(err error) string {
	var errno syscall.Errno
	if !errors.As(err, &errno) {
		return err.Error()
	}
	text := errno.Error()
	return strings.ToUpper(text[:1]) + text[1:]
}

// Returns a card's file name from the card_file rule, such as
// "DEMO-001 Retry failed uploads.md", without the characters Obsidian
// refuses. card set uses it to rename the note.
func MakeCardFileName(rules NameRules, cardKey, title string) (string, error) {
	name := strings.TrimSpace(badNameChars.ReplaceAllString(title, ""))
	if name == "" {
		return "", errors.New("the title needs a letter or digit")
	}
	rule, err := rules.FindNameRule("card_file")
	if err != nil {
		return "", err
	}
	return MakeName(rule, map[string]string{"key": cardKey, "title": name}), nil
}
