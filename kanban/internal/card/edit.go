package card

import (
	"errors"
	"slices"
	"strings"
)

// Replaces one frontmatter field and keeps every other line. A missing
// field goes after the nearest earlier field in Fields that the card has,
// such as sessions after prs, else at the end of the frontmatter.
func SetField(text, name string, value FieldValue) (string, error) {
	lines := strings.Split(text, "\n")
	end := findFrontmatterEnd(lines)
	if end == 0 {
		return "", errors.New("the card has no frontmatter; fix it by hand")
	}
	start, stop, ok := findField(lines, end, name)
	if !ok {
		start, stop = end, end
		for i := slices.Index(Fields, name) - 1; i >= 0; i-- {
			if _, after, found := findField(lines, end, Fields[i]); found {
				start, stop = after, after
				break
			}
		}
	}
	lines = slices.Replace(lines, start, stop, renderField(name, value)...)
	return strings.Join(lines, "\n"), nil
}

// Replaces the first "# " heading with the card key and title, or adds a
// heading after the frontmatter when the card has none.
func SetTitle(text, cardKey, title string) string {
	lines := strings.Split(text, "\n")
	end := findFrontmatterEnd(lines)
	heading := "# " + cardKey + " " + title
	first := 0
	if end > 0 {
		first = end + 1
	}
	for index := first; index < len(lines); index++ {
		if strings.HasPrefix(lines[index], "# ") {
			lines[index] = heading
			return strings.Join(lines, "\n")
		}
	}
	lines = slices.Insert(lines, end+1, "", heading, "---")
	return strings.Join(lines, "\n")
}

// Adds a line after a section's last non-blank line, and first makes the
// section, in Sections order, when the card lacks it.
func AddToSection(text, section, line string) string {
	lines := strings.Split(strings.TrimRight(text, "\n"), "\n")
	if _, ok := findHeading(lines, section); !ok {
		lines = insertSection(lines, section)
	}
	start, stop, _ := findSection(lines, section)
	body := slices.Clone(lines[start:stop])
	for len(body) > 0 && strings.TrimSpace(body[len(body)-1]) == "" {
		body = body[:len(body)-1]
	}
	if len(body) == 0 {
		body = []string{""}
	}
	body = append(body, line)
	if stop < len(lines) {
		body = append(body, "")
	}
	lines = slices.Replace(lines, start, stop, body...)
	return strings.Join(lines, "\n") + "\n"
}

// Renders one field as YAML lines, with each list item in JSON quotes.
func renderField(name string, value FieldValue) []string {
	if !value.IsList {
		return []string{name + ": " + value.Text}
	}
	if len(value.List) == 0 {
		return []string{name + ": []"}
	}
	lines := []string{name + ":"}
	for _, item := range value.List {
		lines = append(lines, "  - "+QuoteASCII(item))
	}
	return lines
}

// Returns the lines a field spans, its indented list items included.
func findField(lines []string, end int, name string) (start, stop int, ok bool) {
	for start = 1; start < end; start++ {
		field := fieldPattern.FindStringSubmatch(lines[start])
		if field == nil || field[1] != name {
			continue
		}
		stop = start + 1
		for stop < end && (strings.HasPrefix(lines[stop], " ") || strings.HasPrefix(lines[stop], "\t")) {
			stop++
		}
		return start, stop, true
	}
	return 0, 0, false
}

// Inserts an empty section before the next section in Sections order
// that the card has, else at the end.
func insertSection(lines []string, section string) []string {
	for _, later := range Sections[slices.Index(Sections, section)+1:] {
		if place, ok := findHeading(lines, later); ok {
			return slices.Insert(lines, place, "## "+section, "---", "")
		}
	}
	return append(lines, "", "## "+section, "---")
}
