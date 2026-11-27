package card

import (
	"regexp"
	"strconv"
	"strings"
	"time"
	"unicode"
	"unicode/utf8"
)

var (
	headingPattern        = regexp.MustCompile(`^(#{1,2})[` + spaceClass + `]+(.*?)[` + spaceClass + `]*$`)
	outlineHeadingPattern = regexp.MustCompile(`^(#{1,3})[` + spaceClass + `]+[^` + spaceClass + `]`)
	itemPrefixPattern     = regexp.MustCompile(`^(- \[[ xX]\] |[-*] )?(.*)$`)
	questionPattern       = regexp.MustCompile(`^Q(\d{1,6})(?: asked (\d{4}-\d\d-\d\d))?: (.*)$`)
)

// One "##" or "###" heading with its sed -n line range, for card outline.
// The range ends before the next heading of its level or higher.
type HeadingRange struct {
	Heading    string // the heading's words, such as Context
	Line       string // the whole heading line, as the text form prints it
	Level      int
	Start, End int
}

// One line card grep found, with its card key and section.
type Match struct{ CardKey, Section, Line string }

// One Open Questions item. Number is its Q number, or the next free one
// when the item has none, and Asked is nil without a valid date.
type Question struct {
	Number int
	Asked  *string
	Text   string
	Lines  []string // the item's later lines, without their two-space indent
}

// Returns a section's text and whether the section exists. The heading
// matches in any case, and one "---" line under it belongs to the heading.
func ReadSection(text, name string) (string, bool) {
	lines := strings.Split(text, "\n")
	start, stop, ok := findSection(lines, name)
	if !ok {
		return "", false
	}
	return strings.Join(lines[start:stop], "\n"), true
}

// Returns the section's name as the card's heading writes it, else as
// Sections writes it, or false when neither has it.
func FindSectionName(text, name string) (string, bool) {
	lines := strings.Split(text, "\n")
	if index, ok := findHeading(lines, name); ok {
		return headingPattern.FindStringSubmatch(lines[index])[2], true
	}
	for _, section := range Sections {
		if strings.EqualFold(section, name) {
			return section, true
		}
	}
	return "", false
}

// Lists Sections, then each other "##" heading of the card in order.
func ListSectionNames(text string) []string {
	lines := strings.Split(text, "\n")
	names := append([]string{}, Sections...)
	seen := map[string]bool{}
	for _, name := range names {
		seen[strings.ToLower(name)] = true
	}
	for _, line := range lines[findFrontmatterEnd(lines)+1:] {
		heading := headingPattern.FindStringSubmatch(line)
		if heading != nil && heading[1] == "##" && !seen[strings.ToLower(heading[2])] {
			names = append(names, heading[2])
			seen[strings.ToLower(heading[2])] = true
		}
	}
	return names
}

// Lists a section's lines for show, with each item's first line numbered
// as "N. " and an empty list when the card lacks the section.
func ListNumberedSectionLines(text, section string) []string {
	lines := strings.Split(text, "\n")
	numbers := map[int]int{}
	for number, item := range listItems(lines, section) {
		numbers[item[0]] = number + 1
	}
	start, stop, _ := findSection(lines, section)
	shown := []string{}
	for index := start; index < stop; index++ {
		if number, ok := numbers[index]; ok {
			shown = append(shown, strconv.Itoa(number)+". "+lines[index])
		} else {
			shown = append(shown, lines[index])
		}
	}
	return shown
}

// Lists each "##" and "###" heading of a card with its line range, in
// order. Lines count as Python's splitlines counts them, so U+2028 in a
// card starts a new line here as in the Python card.
func ListHeadingRanges(text string) []HeadingRange {
	lines := splitPythonLines(text)
	type heading struct{ index, level int }
	headings := []heading{}
	for index := findFrontmatterEnd(lines) + 1; index < len(lines); index++ {
		if found := outlineHeadingPattern.FindStringSubmatch(lines[index]); found != nil {
			headings = append(headings, heading{index: index, level: len(found[1])})
		}
	}
	ranges := []HeadingRange{}
	for position, h := range headings {
		if h.level == 1 {
			continue
		}
		end := len(lines)
		for _, later := range headings[position+1:] {
			if later.level <= h.level {
				end = later.index
				break
			}
		}
		words := strings.TrimSpace(strings.TrimLeft(lines[h.index], "#"))
		ranges = append(ranges, HeadingRange{
			Heading: words,
			Line:    lines[h.index],
			Level:   h.level,
			Start:   h.index + 1,
			End:     end,
		})
	}
	return ranges
}

// Returns each line of each card that pattern matches, in the cards' order.
// Frontmatter lines come under "fields", and headings under "Title".
func FindMatches(cards []Card, pattern *regexp.Regexp) []Match {
	matches := []Match{}
	for _, c := range cards {
		for _, labeled := range labelLines(c.Text) {
			if pattern.MatchString(labeled.Line) {
				matches = append(matches, Match{CardKey: c.CardKey, Section: labeled.Section, Line: labeled.Line})
			}
		}
	}
	return matches
}

// Lists the Open Questions items, numbering each item without a Q number
// after the highest Q number in Open Questions and Decisions.
func ListQuestions(text string) []Question {
	lines := strings.Split(text, "\n")
	questions := []Question{}
	numbered := []bool{}
	highest := findHighestAnsweredNumber(lines)
	for _, item := range listItems(lines, OpenQuestions) {
		question := Question{Text: splitItem(lines[item[0]]), Lines: []string{}}
		for _, line := range lines[item[0]+1 : item[1]] {
			question.Lines = append(question.Lines, strings.TrimPrefix(line, "  "))
		}
		match := questionPattern.FindStringSubmatch(question.Text)
		if match != nil {
			question.Number, _ = strconv.Atoi(match[1])
			question.Text = match[3]
			if _, err := time.Parse(time.DateOnly, match[2]); err == nil {
				question.Asked = &match[2]
			}
			highest = max(highest, question.Number)
		}
		questions = append(questions, question)
		numbered = append(numbered, match != nil)
	}
	for i := range questions {
		if !numbered[i] {
			highest++
			questions[i].Number = highest
		}
	}
	return questions
}

// Pairs each line after the frontmatter with its "##" section, as card
// grep labels them.
func labelLines(text string) []Match {
	lines := strings.Split(text, "\n")
	end := findFrontmatterEnd(lines)
	labeled := []Match{}
	for _, line := range lines[1:max(end, 1)] {
		labeled = append(labeled, Match{Section: "fields", Line: line})
	}
	body := lines
	if end > 0 {
		body = lines[end+1:]
	}
	section := "Title"
	for _, line := range body {
		heading := headingPattern.FindStringSubmatch(line)
		switch {
		case heading != nil && heading[1] == "##":
			section = heading[2]
		case heading != nil:
			labeled = append(labeled, Match{Section: "Title", Line: heading[2]})
		case strings.TrimSpace(line) != "" && strings.TrimSpace(line) != "---":
			labeled = append(labeled, Match{Section: section, Line: line})
		}
	}
	return labeled
}

// Returns the index of the "##" heading with this name, in any case.
func findHeading(lines []string, name string) (int, bool) {
	for index := findFrontmatterEnd(lines) + 1; index < len(lines); index++ {
		heading := headingPattern.FindStringSubmatch(lines[index])
		if heading != nil && heading[1] == "##" && strings.EqualFold(heading[2], name) {
			return index, true
		}
	}
	return 0, false
}

// Returns the line range of a section's body, after its heading and one
// "---" line, up to the next "#" or "##" heading.
func findSection(lines []string, name string) (start, stop int, ok bool) {
	heading, ok := findHeading(lines, name)
	if !ok {
		return 0, 0, false
	}
	start = heading + 1
	if start < len(lines) && strings.TrimSpace(lines[start]) == "---" {
		start++
	}
	stop = start
	for stop < len(lines) && !headingPattern.MatchString(lines[stop]) {
		stop++
	}
	return start, stop, true
}

// Lists each item of a section as its [first, after-last) line range. An
// item is an unindented line and the indented lines right after it.
func listItems(lines []string, section string) [][2]int {
	start, stop, ok := findSection(lines, section)
	if !ok {
		return nil
	}
	items := [][2]int{}
	for index := start; index < stop; index++ {
		line := lines[index]
		first, _ := utf8.DecodeRuneInString(line)
		switch {
		case strings.TrimSpace(line) == "":
		case !unicode.IsSpace(first):
			items = append(items, [2]int{index, index + 1})
		case len(items) > 0 && items[len(items)-1][1] == index:
			items[len(items)-1][1] = index + 1
		}
	}
	return items
}

// Returns an item's words without its "- [ ] " or "- " prefix.
func splitItem(line string) string {
	return itemPrefixPattern.FindStringSubmatch(line)[2]
}

// Returns the highest Q number of an answered question in Decisions, or 0.
func findHighestAnsweredNumber(lines []string) int {
	highest := 0
	for _, item := range listItems(lines, Decisions) {
		if match := questionPattern.FindStringSubmatch(splitItem(lines[item[0]])); match != nil {
			number, _ := strconv.Atoi(match[1])
			highest = max(highest, number)
		}
	}
	return highest
}

// Splits text into lines as Python's str.splitlines does, with no empty
// line after a final line break.
func splitPythonLines(text string) []string {
	lines := []string{}
	start := 0
	for index, r := range text {
		switch r {
		case '\n', '\r', '\v', '\f', '\x1c', '\x1d', '\x1e', '\u0085', '\u2028', '\u2029':
			lines = append(lines, text[start:index])
			start = index + utf8.RuneLen(r)
		}
	}
	if start < len(text) {
		lines = append(lines, text[start:])
	}
	return lines
}
