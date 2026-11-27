package card

import (
	"errors"
	"fmt"
	"regexp"
	"strconv"
	"strings"
)

var questionKeyPattern = regexp.MustCompile(`^[Qq](\d+)$`)

// One item card edit changes, with its first line as card read it. An edit
// refuses the item once that line has changed.
type Item struct {
	CardKey string
	Section string
	Given   string // N as typed, such as "2", or "Q4" in Open Questions
	Line    string
}

// Finds item N of a section, as card show numbers it. In Open Questions it
// first writes each unnumbered item's Q number, as every edit there does.
func FindItem(text, cardKey, section, given string) (Item, error) {
	item := Item{CardKey: cardKey, Section: section, Given: given}
	lines, bounds, err := findItemLines(text, item)
	if err != nil {
		return Item{}, err
	}
	item.Line = lines[bounds[0]]
	return item, nil
}

// Returns the item's words without its bullet or checkbox.
func (i Item) Words() string {
	return splitItem(i.Line)
}

// Replaces the item's words. It keeps the item's own bullet or checkbox,
// and in Open Questions its `Q4 asked DATE: ` head.
func ReplaceItem(text string, item Item, newText string) (string, error) {
	lines, bounds, err := findUnchangedItem(text, item)
	if err != nil {
		return "", err
	}
	prefix, oldWords := SplitBullet(item.Line)
	givenPrefix, words := SplitBullet(strings.TrimSpace(newText))
	if item.Section == OpenQuestions {
		words = keepQuestionHead(oldWords, words)
	}
	if prefix == "" {
		prefix = givenPrefix
	}
	lines[bounds[0]] = prefix + words
	return strings.Join(lines, "\n"), nil
}

// Removes the item with its indented lines, keeping one blank line where
// it was.
func RemoveItem(text string, item Item) (string, error) {
	lines, bounds, err := findUnchangedItem(text, item)
	if err != nil {
		return "", err
	}
	start := bounds[0]
	lines = append(lines[:start], lines[bounds[1]:]...)
	if start > 0 && start < len(lines) && strings.TrimSpace(lines[start-1]) == "" && strings.TrimSpace(lines[start]) == "" {
		lines = append(lines[:start], lines[start+1:]...)
	}
	return strings.Join(lines, "\n"), nil
}

// Ticks or unticks the item's checkbox and keeps its words. It refuses an
// item with no checkbox, or one already in that state.
func TickItem(text string, item Item, ticked bool) (string, error) {
	lines, bounds, err := findUnchangedItem(text, item)
	if err != nil {
		return "", err
	}
	box, verb := "- [ ] ", "unticked"
	if ticked {
		box, verb = "- [x] ", "ticked"
	}
	prefix, words := SplitBullet(item.Line)
	label := item.Section + " item " + item.Given
	if !strings.HasPrefix(prefix, "- [") {
		return "", fmt.Errorf("%s %s has no checkbox", item.CardKey, label)
	}
	if strings.ToLower(prefix) == box {
		return "", fmt.Errorf("%s %s is already %s", item.CardKey, label, verb)
	}
	lines[bounds[0]] = box + words
	return strings.Join(lines, "\n"), nil
}

// Splits an item line into its bullet or checkbox, such as "- [ ] ", and
// its words. The bullet is "" for a plain line.
func SplitBullet(line string) (bullet, words string) {
	match := itemPrefixPattern.FindStringSubmatch(line)
	return match[1], match[2]
}

// Writes each unnumbered Open Questions item's shown Q number into it, so
// a later add or removal cannot shift it. For example `- b` after
// `- Q3: a` becomes `- Q4: b`.
func StampQuestionNumbers(text string) string {
	lines := strings.Split(text, "\n")
	questions := ListQuestions(text)
	for i, item := range listItems(lines, OpenQuestions) {
		bullet, words := SplitBullet(lines[item[0]])
		if questionPattern.MatchString(words) {
			continue
		}
		if bullet == "" {
			bullet = "- "
		}
		lines[item[0]] = fmt.Sprintf("%sQ%d: %s", bullet, questions[i].Number, words)
	}
	return strings.Join(lines, "\n")
}

// Returns the card's highest Q number, open or answered, or 0. Decisions
// items keep their `Q3 asked DATE:` head, so no number is given twice.
func FindHighestQuestionNumber(text string) int {
	highest := findHighestAnsweredNumber(strings.Split(text, "\n"))
	for _, question := range ListQuestions(text) {
		highest = max(highest, question.Number)
	}
	return highest
}

// Finds the item again, as FindItem does, and refuses it when its first
// line is no longer the one card read.
func findUnchangedItem(text string, item Item) ([]string, [2]int, error) {
	lines, bounds, err := findItemLines(text, item)
	if err != nil {
		return nil, bounds, err
	}
	if lines[bounds[0]] != item.Line {
		return nil, bounds, fmt.Errorf("%s %s item %s changed; run card show again", item.CardKey, item.Section, item.Given)
	}
	return lines, bounds, nil
}

// Returns the text's lines, after stamping Q numbers in Open Questions,
// and the line range of the item that item.Given names.
func findItemLines(text string, item Item) ([]string, [2]int, error) {
	if item.Section == OpenQuestions {
		text = StampQuestionNumbers(text)
	}
	position, err := findItemPosition(text, item)
	if err != nil {
		return nil, [2]int{}, err
	}
	lines := strings.Split(text, "\n")
	items := listItems(lines, item.Section)
	if position < 1 || position > len(items) {
		return nil, [2]int{}, fmt.Errorf("%s %s has %d items, so no item %d", item.CardKey, item.Section, len(items), position)
	}
	return lines, items[position-1], nil
}

// Returns the position, from 1, of the item that item.Given names, such
// as "2", or a Q number such as "Q4" in Open Questions.
func findItemPosition(text string, item Item) (int, error) {
	if isASCIIDigits(item.Given) {
		return strconv.Atoi(item.Given)
	}
	match := questionKeyPattern.FindStringSubmatch(item.Given)
	if item.Section != OpenQuestions || match == nil {
		return 0, errors.New("N is an item number, or Q<n> in Open Questions") //nolint:staticcheck // the Python card's words
	}
	number, _ := strconv.Atoi(match[1])
	for index, question := range ListQuestions(text) {
		if question.Number == number {
			return index + 1, nil
		}
	}
	return 0, fmt.Errorf("%s %s has no Q%s", item.CardKey, item.Section, match[1])
}

// Returns the new words behind the old item's `Q4 asked DATE: ` head. New
// words that bring their own head stay as given.
func keepQuestionHead(oldWords, newWords string) string {
	head := questionPattern.FindStringSubmatchIndex(oldWords)
	if head == nil || questionPattern.MatchString(newWords) {
		return newWords
	}
	return oldWords[:head[6]] + newWords
}

// Reports whether text is one or more of the digits 0 to 9, so "+2" is no
// item number.
func isASCIIDigits(text string) bool {
	return text != "" && strings.Trim(text, "0123456789") == ""
}
