package card

import (
	"regexp"
	"strings"
)

// Go's \s misses Unicode spaces such as U+00A0, which Python's \s matches.
// Patterns that port a Python \s put this class in brackets instead.
const spaceClass = `\t-\r\x{1c}-\x{20}\x{85}\p{Z}`

var (
	fieldPattern    = regexp.MustCompile(`^([A-Za-z_][\p{L}\p{N}_-]*):(.*)$`)
	listItemPattern = regexp.MustCompile(`^[` + spaceClass + `]+-[` + spaceClass + `]+(.*)$`)
	inlineItem      = regexp.MustCompile(`"[^"]*"|'[^']*'|[^,` + spaceClass + `][^,]*`)
)

// One frontmatter value, a plain string or a list for fields such as plans.
type FieldValue struct {
	Text   string
	List   []string
	IsList bool
}

// Treats a missing or empty field as an empty list, and a plain value as a
// list of one.
func (v FieldValue) AsList() []string {
	if v.IsList {
		return v.List
	}
	if v.Text == "" {
		return []string{}
	}
	return []string{v.Text}
}

// Reads the frontmatter fields card uses. It reads the YAML subset card
// writes and Obsidian's property editor writes, not all of YAML.
func ReadFields(text string) map[string]FieldValue {
	lines := strings.Split(text, "\n")
	fields := map[string]FieldValue{}
	name := ""
	for _, line := range lines[1:max(findFrontmatterEnd(lines), 1)] {
		if field := fieldPattern.FindStringSubmatch(line); field != nil {
			name = field[1]
			fields[name] = parseValue(strings.TrimSpace(field[2]))
		} else if item := listItemPattern.FindStringSubmatch(line); item != nil && name != "" {
			list := append(fields[name].AsList(), unquote(item[1]))
			fields[name] = FieldValue{List: list, IsList: true}
		}
	}
	return fields
}

// Returns the index of the line that closes the frontmatter, or 0 when the
// text has none, as the Python card does.
func findFrontmatterEnd(lines []string) int {
	if len(lines) == 0 || strings.TrimSpace(lines[0]) != "---" {
		return 0
	}
	for index := 1; index < len(lines); index++ {
		if strings.TrimSpace(lines[index]) == "---" {
			return index
		}
	}
	return 0
}

// Parses a plain value or an inline list such as ["a", b].
func parseValue(value string) FieldValue {
	if !strings.HasPrefix(value, "[") || !strings.HasSuffix(value, "]") {
		return FieldValue{Text: unquote(value)}
	}
	inner := strings.TrimSuffix(strings.TrimPrefix(value, "["), "]")
	list := []string{}
	for _, item := range inlineItem.FindAllString(inner, -1) {
		list = append(list, unquote(item))
	}
	return FieldValue{List: list, IsList: true}
}

// Strips one pair of matching quotes, such as "a" or 'a'.
func unquote(value string) string {
	value = strings.TrimSpace(value)
	if len(value) >= 2 && value[0] == value[len(value)-1] && strings.ContainsRune(`"'`, rune(value[0])) {
		return value[1 : len(value)-1]
	}
	return value
}
