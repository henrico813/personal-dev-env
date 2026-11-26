package card

import (
	"fmt"
	"regexp"
	"slices"
	"strings"
	"unicode"
	"unicode/utf8"
)

// Characters a file name cannot hold, in Obsidian or on Windows.
var badNameChars = regexp.MustCompile(`[\\/:*?"<>|#^\[\]]`)

// The sections a version 1 card lacks, which check does not report.
var versionTwoSections = []string{"Notes", "Appendix"}

// One broken title rule. Card prints its rule, name and detail on one line.
type TitleFinding struct{ Rule, Name, Detail string }

// Formats the finding as card prints it, such as `T2 label: "board:"`.
func (f TitleFinding) String() string {
	return fmt.Sprintf("%s %s: %s", f.Rule, f.Name, f.Detail)
}

// Lists what is wrong with one card's fields, sections and title. Done and
// dropped cards keep the titles they closed with.
func FindIssues(rules NameRules, c Card) []string {
	lines := strings.Split(c.Text, "\n")
	if findFrontmatterEnd(lines) == 0 {
		return []string{"has no frontmatter"}
	}
	issues := []string{}
	if stage := c.ReadStage(); !slices.Contains(Stages, stage) {
		issues = append(issues, "unknown stage "+quotePythonRepr(stage))
	}
	if kind := c.ReadKind(); !slices.Contains(Kinds, kind) {
		issues = append(issues, "unknown kind "+quotePythonRepr(kind))
	}
	for _, name := range Sections {
		if _, found := findHeading(lines, name); !found && !slices.Contains(versionTwoSections, name) {
			issues = append(issues, "no "+name+" section")
		}
	}
	if stage := c.ReadStage(); stage != Done && stage != Dropped {
		for _, finding := range CheckTitle(rules, c.ReadTitle()) {
			issues = append(issues, finding.String())
		}
	}
	return issues
}

// Runs the title rules T1 to T4 and returns each one the title breaks.
// Card refuses a title with any finding.
func CheckTitle(rules NameRules, title string) []TitleFinding {
	findings := []TitleFinding{}
	if length := utf8.RuneCountInString(title); length > TitleLimit {
		detail := fmt.Sprintf("%d characters, at most %d", length, TitleLimit)
		findings = append(findings, TitleFinding{Rule: "T1", Name: "length", Detail: detail})
	}
	words := strings.Split(title, " ")
	for _, word := range words[:min(3, len(words))] {
		if strings.HasSuffix(word, ":") {
			findings = append(findings, TitleFinding{Rule: "T2", Name: "label", Detail: `"` + word + `"`})
			break
		}
	}
	if keys := findTitleKeys(rules, title); len(keys) > 0 {
		detail := `"` + strings.Join(keys, `", "`) + `"`
		findings = append(findings, TitleFinding{Rule: "T3", Name: "key", Detail: detail})
	}
	form := []string{}
	if first, _ := utf8.DecodeRuneInString(title); unicode.IsLower(first) {
		form = append(form, "starts with a lower-case letter")
	}
	if strings.HasSuffix(title, ".") {
		form = append(form, "ends with a period")
	}
	// A label's colon is a T2 finding only.
	body := slices.Clone(words)
	for index := range body[:min(3, len(body))] {
		body[index] = strings.TrimSuffix(body[index], ":")
	}
	refused := badNameChars.FindAllString(strings.Join(body, " "), -1)
	slices.Sort(refused)
	if refused = slices.Compact(refused); len(refused) > 0 {
		form = append(form, "holds "+strings.Join(refused, " "))
	}
	if len(form) > 0 {
		findings = append(findings, TitleFinding{Rule: "T4", Name: "form", Detail: strings.Join(form, ", ")})
	}
	return findings
}

// Lists the title's words that read as a card key or a plan key, in any
// case, such as DEMO-001.
func findTitleKeys(rules NameRules, title string) []string {
	cardKey, _ := rules.FindNameRule("card_key")
	planFile, _ := rules.FindNameRule("plan_file")
	keys := []string{}
	for _, word := range strings.Fields(title) {
		word = strings.Trim(word, ".,:;!?()\"'")
		// plan_file reads a whole file name, so the word gets its suffix.
		if matchesWholeIgnoringCase(cardKey, word) || matchesWholeIgnoringCase(planFile, word+".md") {
			keys = append(keys, word)
		}
	}
	return keys
}

// Reports whether the rule's pattern, in any case, matches all of text.
func matchesWholeIgnoringCase(rule NameRule, text string) bool {
	if rule.Parse == nil {
		return false
	}
	pattern := regexp.MustCompile("(?i)" + rule.Parse.String())
	match := pattern.FindStringIndex(text)
	return match != nil && match[0] == 0 && match[1] == len(text)
}

// Quotes text as Python's repr does for a str, such as 'later', "it's" or
// 'a\tb\xa0c'.
func quotePythonRepr(text string) string {
	quote := "'"
	if strings.ContainsRune(text, '\'') && !strings.ContainsRune(text, '"') {
		quote = `"`
	}
	var quoted strings.Builder
	quoted.WriteString(quote)
	for _, r := range text {
		switch {
		case string(r) == quote || r == '\\':
			quoted.WriteString(`\` + string(r))
		case r == '\t':
			quoted.WriteString(`\t`)
		case r == '\n':
			quoted.WriteString(`\n`)
		case r == '\r':
			quoted.WriteString(`\r`)
		case r < ' ' || r == 0x7f:
			fmt.Fprintf(&quoted, `\x%02x`, r)
		case r < 0x7f || unicode.IsPrint(r):
			quoted.WriteRune(r)
		case r <= 0xff:
			fmt.Fprintf(&quoted, `\x%02x`, r)
		case r <= 0xffff:
			fmt.Fprintf(&quoted, `\u%04x`, r)
		default:
			fmt.Fprintf(&quoted, `\U%08x`, r)
		}
	}
	quoted.WriteString(quote)
	return quoted.String()
}
