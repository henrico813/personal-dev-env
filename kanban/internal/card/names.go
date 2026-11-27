package card

import (
	"fmt"
	"os"
	"regexp"
	"slices"
	"strings"
	"unicode/utf8"

	"github.com/BurntSushi/toml"
)

// The names.toml version this card reads.
const rulesVersion = 1

// A field in a template, such as {key|lower}, with an optional filter.
var slotPattern = regexp.MustCompile(`\{([a-z_]+)(?:\|([a-z0-9]+))?\}`)

var slugWord = regexp.MustCompile(`[a-z0-9]+`)

// The filters a template field may name, in the order error messages list
// them.
var filterNames = []string{"lower", "slug", "pad3"}

var filters = map[string]func(string) string{
	"lower": strings.ToLower,
	"slug": func(text string) string {
		return strings.Join(slugWord.FindAllString(strings.ToLower(text), -1), "-")
	},
	"pad3": func(text string) string {
		return strings.Repeat("0", max(0, 3-utf8.RuneCountInString(text))) + text
	},
}

// One naming rule from names.toml. Make builds a name and Parse reads it back.
type NameRule struct {
	Name  string
	Make  string
	Parse *regexp.Regexp
}

// Every naming rule by name, and the file they came from.
type NameRules struct {
	path  string
	rules map[string]NameRule
}

// Parses names.toml, checks its version, and compiles each pattern. The
// file's (?P<name>...) groups compile in Go's regexp unchanged.
func ReadNameRules(path string) (NameRules, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return NameRules{}, fmt.Errorf("cannot read %s: %w", path, err)
	}
	var tables map[string]any
	meta, err := toml.Decode(string(data), &tables)
	if err != nil {
		return NameRules{}, fmt.Errorf("cannot read %s: %w", path, err)
	}
	version, ok := tables["version"]
	if !ok {
		version = "unset"
	}
	if version != int64(rulesVersion) {
		return NameRules{}, fmt.Errorf("%s is version %v; this card reads version %d", path, version, rulesVersion)
	}
	rules := NameRules{path: path, rules: map[string]NameRule{}}
	for _, key := range meta.Keys() {
		name := key.String()
		if len(key) != 1 || name == "version" {
			continue
		}
		table, ok := tables[name].(map[string]any)
		if !ok {
			return NameRules{}, fmt.Errorf("%s: %s is not a [%s] table", path, name, name)
		}
		rule, err := buildNameRule(path, name, table)
		if err != nil {
			return NameRules{}, err
		}
		rules.rules[name] = rule
	}
	return rules, nil
}

// Finds one naming rule, or fails with a message that names the file.
func (r NameRules) FindNameRule(name string) (NameRule, error) {
	rule, ok := r.rules[name]
	if !ok {
		return NameRule{}, fmt.Errorf("%s has no naming rule %s", r.path, name)
	}
	return rule, nil
}

// Lists the fields the rule's template names, each once, in the order they
// first appear, such as [type words] for "{type}/{words|slug}".
func (r NameRule) ListFields() []string {
	fields := []string{}
	for _, slot := range slotPattern.FindAllStringSubmatch(r.Make, -1) {
		if !slices.Contains(fields, slot[1]) {
			fields = append(fields, slot[1])
		}
	}
	return fields
}

// Fills a rule's template and runs each field's filter, which is lower, slug
// or pad3. A field with no value fills as an empty string.
func MakeName(rule NameRule, values map[string]string) string {
	return slotPattern.ReplaceAllStringFunc(rule.Make, func(slot string) string {
		parts := slotPattern.FindStringSubmatch(slot)
		value := values[parts[1]]
		if parts[2] == "" {
			return value
		}
		return filters[parts[2]](value)
	})
}

// Reads a name's fields with the rule's pattern, which must match the whole
// name, as each names.toml pattern starts with ^ and ends with $. Groups
// that matched nothing are left out.
func ParseName(rule NameRule, name string) (map[string]string, bool) {
	match := rule.Parse.FindStringSubmatchIndex(name)
	if match == nil || match[0] != 0 || match[1] != len(name) {
		return nil, false
	}
	fields := map[string]string{}
	for index, field := range rule.Parse.SubexpNames() {
		if field != "" && match[2*index] >= 0 {
			fields[field] = name[match[2*index]:match[2*index+1]]
		}
	}
	return fields, true
}

// Reads the card key a file name holds under the card_file rule. A plan's
// name can hold one too, so readCardNote also checks the type field.
func ReadCardKey(rules NameRules, fileName string) (string, bool) {
	rule, err := rules.FindNameRule("card_file")
	if err != nil {
		return "", false
	}
	fields, ok := ParseName(rule, fileName)
	if !ok || fields["key"] == "" {
		return "", false
	}
	return fields["key"], true
}

// Makes a NameRule from its table, checking its filters and pattern.
func buildNameRule(path, name string, table map[string]any) (NameRule, error) {
	template, makeOK := table["make"].(string)
	parse, parseOK := table["parse"].(string)
	if !makeOK || !parseOK {
		return NameRule{}, fmt.Errorf("%s: rule %s needs make and parse", path, name)
	}
	for _, slot := range slotPattern.FindAllStringSubmatch(template, -1) {
		if _, known := filters[slot[2]]; slot[2] != "" && !known {
			return NameRule{}, fmt.Errorf("%s: rule %s has unknown filter %s; filters are %s",
				path, name, slot[2], strings.Join(filterNames, ", "))
		}
	}
	pattern, err := regexp.Compile(parse)
	if err != nil {
		return NameRule{}, fmt.Errorf("%s: rule %s has a bad pattern %q: %w", path, name, parse, err)
	}
	return NameRule{Name: name, Make: template, Parse: pattern}, nil
}
