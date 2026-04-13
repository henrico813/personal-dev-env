// Package inspect provides markdown parsing for canonical planner-rendered plans.
// It reconstructs schema.Plan from markdown and returns typed section/step spans
// for splice-based partial replacement.
package inspect

import (
	"fmt"
	"strings"

	"planner/schema"
)

// Span represents a byte range in the source markdown document.
type Span struct {
	Start int `json:"start"`
	End   int `json:"end"`
}

// SectionSpans holds the byte ranges for each top-level plan section.
type SectionSpans struct {
	Overview         Span `json:"overview"`
	DefinitionOfDone Span `json:"definition_of_done"`
	Implementation   Span `json:"implementation"`
	Verification     Span `json:"verification"`
}

// ParseMarkdown parses canonical planner-rendered markdown and returns the
// reconstructed Plan, typed section spans, and per-step spans for the
// implementation section. Returns an error if the input does not match
// canonical format (fail closed on drift).
func ParseMarkdown(input string) (schema.Plan, SectionSpans, []Span, error) {
	if strings.Contains(input, "\r") {
		return schema.Plan{}, SectionSpans{}, nil, fmt.Errorf("CRLF line endings not supported; convert to LF")
	}
	if !strings.HasPrefix(input, "# ") {
		return schema.Plan{}, SectionSpans{}, nil, fmt.Errorf("missing title heading")
	}

	sectionSpans, err := findTopLevelSections(input)
	if err != nil {
		return schema.Plan{}, SectionSpans{}, nil, err
	}

	plan := schema.Plan{}
	titleLine := strings.SplitN(input, "\n", 2)[0]
	plan.Title = strings.TrimSpace(strings.TrimPrefix(titleLine, "# "))

	overviewBody, _, err := sectionBody(input, sectionSpans["Overview"])
	if err != nil {
		return schema.Plan{}, SectionSpans{}, nil, err
	}
	plan.Overview = strings.TrimSpace(overviewBody)

	dodBody, _, err := sectionBody(input, sectionSpans["Definition of Done"])
	if err != nil {
		return schema.Plan{}, SectionSpans{}, nil, err
	}
	parsedDoD, err := parseDefinitionOfDone(dodBody)
	if err != nil {
		return schema.Plan{}, SectionSpans{}, nil, err
	}
	plan.DefinitionOfDone = parsedDoD

	implBody, implBodyStart, err := sectionBody(input, sectionSpans["Implementation"])
	if err != nil {
		return schema.Plan{}, SectionSpans{}, nil, err
	}
	steps, stepSpans, err := parseImplementation(implBody, implBodyStart)
	if err != nil {
		return schema.Plan{}, SectionSpans{}, nil, err
	}
	plan.Implementation = steps

	verificationBody, _, err := sectionBody(input, sectionSpans["Verification"])
	if err != nil {
		return schema.Plan{}, SectionSpans{}, nil, err
	}
	parsedVerification, err := parseVerification(verificationBody)
	if err != nil {
		return schema.Plan{}, SectionSpans{}, nil, err
	}
	plan.Verification = parsedVerification

	return plan, toSectionSpans(sectionSpans), stepSpans, nil
}

func findTopLevelSections(input string) (map[string]Span, error) {
	headings := scanHeadings(input, "## ")
	filtered := []headingMatch{}
	for _, h := range headings {
		switch h.Text {
		case "Overview", "Definition of Done", "Implementation", "Verification":
			filtered = append(filtered, h)
		}
	}

	if len(filtered) != 4 {
		return nil, fmt.Errorf("expected 4 top-level sections, got %d", len(filtered))
	}

	spans := map[string]Span{}
	for i, h := range filtered {
		end := len(input)
		if i+1 < len(filtered) {
			end = filtered[i+1].Start
		}
		spans[h.Text] = Span{Start: h.Start, End: end}
	}

	for _, required := range []string{"Overview", "Definition of Done", "Implementation", "Verification"} {
		if _, ok := spans[required]; !ok {
			return nil, fmt.Errorf("missing section %q", required)
		}
	}

	return spans, nil
}

func toSectionSpans(spans map[string]Span) SectionSpans {
	return SectionSpans{
		Overview:         spans["Overview"],
		DefinitionOfDone: spans["Definition of Done"],
		Implementation:   spans["Implementation"],
		Verification:     spans["Verification"],
	}
}

func sectionBody(input string, span Span) (string, int, error) {
	if span.Start < 0 || span.End > len(input) || span.Start >= span.End {
		return "", 0, fmt.Errorf("invalid section span")
	}
	section := input[span.Start:span.End]

	headingEnd := strings.Index(section, "\n")
	if headingEnd < 0 {
		return "", 0, fmt.Errorf("section heading has no newline")
	}

	divider := "\n---\n"
	dividerIndex := strings.Index(section, divider)
	if dividerIndex < 0 {
		return "", 0, fmt.Errorf("section missing divider")
	}
	if dividerIndex != headingEnd {
		return "", 0, fmt.Errorf("section divider not immediately after heading")
	}

	bodyStart := span.Start + dividerIndex + len(divider)
	if bodyStart < span.End && input[bodyStart] == '\n' {
		bodyStart++
	}
	return input[bodyStart:span.End], bodyStart, nil
}

type headingMatch struct {
	Start int
	Text  string
}

func scanHeadings(input string, prefix string) []headingMatch {
	matches := []headingMatch{}
	inFence := false
	fence := ""
	pos := 0
	for _, line := range strings.SplitAfter(input, "\n") {
		trimmed := strings.TrimRight(line, "\n")
		if maybeFence, ok := parseFence(trimmed); ok {
			if !inFence {
				inFence = true
				fence = maybeFence
			} else if maybeFence == fence {
				inFence = false
				fence = ""
			}
		}
		if !inFence && strings.HasPrefix(trimmed, prefix) {
			matches = append(matches, headingMatch{
				Start: pos,
				Text:  strings.TrimPrefix(trimmed, prefix),
			})
		}
		pos += len(line)
	}
	return matches
}

func parseDefinitionOfDone(body string) (schema.DefinitionOfDone, error) {
	parts := strings.Split(body, "### Goals")
	if len(parts) != 2 {
		return schema.DefinitionOfDone{}, fmt.Errorf("definition of done missing goals")
	}
	narrative := strings.TrimSpace(parts[0])
	goalsAndRest := strings.Split(parts[1], "### Current State")
	if len(goalsAndRest) != 2 {
		return schema.DefinitionOfDone{}, fmt.Errorf("definition of done missing current state")
	}
	stateAndShape := strings.Split(goalsAndRest[1], "### Module Shape")
	if len(stateAndShape) != 2 {
		return schema.DefinitionOfDone{}, fmt.Errorf("definition of done missing module shape")
	}

	goals := []string{}
	for _, line := range strings.Split(goalsAndRest[0], "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "- [ ] ") {
			goals = append(goals, strings.TrimSpace(strings.TrimPrefix(trimmed, "- [ ] ")))
		}
	}

	return schema.DefinitionOfDone{
		Narrative:    narrative,
		Goals:        goals,
		CurrentState: strings.TrimSpace(stateAndShape[0]),
		ModuleShape:  strings.TrimSpace(stateAndShape[1]),
	}, nil
}

func parseImplementation(body string, base int) ([]schema.Step, []Span, error) {
	steps := []schema.Step{}
	spans := []Span{}

	rawHeadings := scanHeadings(body, "### ")
	headings := []headingMatch{}
	for _, h := range rawHeadings {
		if parts := strings.SplitN(h.Text, ". ", 2); len(parts) == 2 {
			headings = append(headings, headingMatch{Start: h.Start, Text: parts[1]})
		}
	}

	if len(headings) == 0 {
		return nil, nil, fmt.Errorf("implementation section has no steps")
	}

	for i, h := range headings {
		end := len(body)
		if i+1 < len(headings) {
			end = headings[i+1].Start
		}
		chunk := strings.TrimSpace(body[h.Start:end])
		step, err := parseStepChunk(chunk, h.Text)
		if err != nil {
			return nil, nil, err
		}
		steps = append(steps, step)
		spans = append(spans, Span{Start: base + h.Start, End: base + end})
	}

	return steps, spans, nil
}

func parseStepChunk(chunk string, title string) (schema.Step, error) {
	lines := strings.Split(chunk, "\n")
	if len(lines) < 2 {
		return schema.Step{}, fmt.Errorf("invalid implementation step block")
	}

	summaryLines := []string{}
	changes := []schema.FileChange{}
	i := 1
	for i < len(lines) {
		if strings.HasPrefix(strings.TrimSpace(lines[i]), "`") {
			break
		}
		summaryLines = append(summaryLines, lines[i])
		i++
	}

	for i < len(lines) {
		line := strings.TrimSpace(lines[i])
		if !strings.HasPrefix(line, "`") {
			i++
			continue
		}
		filename := strings.Trim(line, "`")
		i++
		explanation := ""
		if i < len(lines) && strings.HasPrefix(strings.TrimSpace(lines[i]), "> ") {
			explanation = strings.TrimPrefix(strings.TrimSpace(lines[i]), "> ")
			i++
		}

		for i < len(lines) && !strings.HasPrefix(strings.TrimSpace(lines[i]), "```") {
			i++
		}
		if i >= len(lines) {
			return schema.Step{}, fmt.Errorf("missing diff fence for file %q", filename)
		}
		fence, ok := parseFence(strings.TrimSpace(lines[i]))
		if !ok {
			return schema.Step{}, fmt.Errorf("invalid fence line %q", lines[i])
		}
		i++
		diffLines := []string{}
		for i < len(lines) && strings.TrimSpace(lines[i]) != fence {
			diffLines = append(diffLines, lines[i])
			i++
		}
		if i >= len(lines) {
			return schema.Step{}, fmt.Errorf("unterminated diff fence for file %q", filename)
		}
		i++

		changes = append(changes, schema.FileChange{
			Filename:    filename,
			Explanation: explanation,
			Diff:        strings.TrimRight(strings.Join(diffLines, "\n"), "\n"),
		})
	}

	return schema.Step{Title: title, Summary: strings.TrimSpace(strings.Join(summaryLines, "\n")), FileChanges: changes}, nil
}

func parseVerification(body string) (*schema.Verification, error) {
	parts := strings.Split(body, "### Automated Verification")
	if len(parts) != 2 {
		return nil, fmt.Errorf("missing automated verification section")
	}
	manual := strings.Split(parts[1], "### Manual Verification")
	if len(manual) != 2 {
		return nil, fmt.Errorf("missing manual verification section")
	}

	return &schema.Verification{
		Summary:   strings.TrimSpace(parts[0]),
		Automated: parseChecklist(manual[0]),
		Manual:    parseChecklist(manual[1]),
	}, nil
}

func parseChecklist(raw string) []string {
	items := []string{}
	for _, line := range strings.Split(raw, "\n") {
		trimmed := strings.TrimSpace(line)
		if strings.HasPrefix(trimmed, "- [ ] ") {
			items = append(items, strings.TrimSpace(strings.TrimPrefix(trimmed, "- [ ] ")))
		}
	}
	return items
}

func parseFence(line string) (string, bool) {
	trimmed := strings.TrimSpace(line)
	if !strings.HasPrefix(trimmed, "```") {
		return "", false
	}
	runes := []rune(trimmed)
	i := 0
	for i < len(runes) && runes[i] == '`' {
		i++
	}
	if i < 3 {
		return "", false
	}
	return string(runes[:i]), true
}
