package internal

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestParseMarkdownRoundTripFromRenderPlan(t *testing.T) {
	plan := Plan{
		Title:    "Plan",
		Overview: "Overview text.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []ChecklistItem{{Text: "One goal"}},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: []Step{
			{
				Title:   "First",
				Summary: "summary one",
				FileChanges: []FileChange{{
					Filename:    "a.txt",
					Explanation: "explain",
					Diff:        "@@ -1 +1 @@\n-old\n+new",
				}},
			},
			{
				Title:   "Second",
				Summary: "summary two",
				FileChanges: []FileChange{{
					Filename:    "b.txt",
					Explanation: "explain",
					Diff:        "@@ -1 +1 @@\n-old\n+new",
				}},
			},
		},
		Verification: &Verification{
			Summary:   "Verification summary.",
			Automated: []ChecklistItem{{Text: "go test ./..."}},
			Manual:    []ChecklistItem{{Text: "smoke"}},
		},
	}

	md, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}

	result, err := ParseMarkdown(md)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	parsed := result.Plan
	stepSpans := result.Steps
	if !reflect.DeepEqual(parsed, plan) {
		t.Fatalf("parsed plan mismatch:\nparsed=%#v\nwant=%#v", parsed, plan)
	}

	if len(stepSpans) != len(plan.Implementation) {
		t.Fatalf("expected %d step spans, got %d", len(plan.Implementation), len(stepSpans))
	}
	firstStepRaw := md[stepSpans[0].Start:stepSpans[0].End]
	if !strings.HasPrefix(strings.TrimSpace(firstStepRaw), "### 1. First") {
		t.Fatalf("step span does not point at first step heading")
	}
}

func TestInspectViewEmitsUpdateDiffExpect(t *testing.T) {
	plan := twoStepPlan()
	md, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	parsed, err := ParseMarkdown(md)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	sectionSpans := parsed.Sections
	view := buildInspectPlan(parsed, md)
	if view.Implementation[0].FileChanges[0].Selector != "implementation[1].file_changes[1]" {
		t.Fatalf("selector=%q", view.Implementation[0].FileChanges[0].Selector)
	}
	if !strings.HasPrefix(view.Implementation[0].FileChanges[0].UpdateDiffExpect, "sha256:") {
		t.Fatalf("token=%q", view.Implementation[0].FileChanges[0].UpdateDiffExpect)
	}
	if view.Implementation[0].FileChanges[0].UpdateDiffExpect == view.Implementation[1].FileChanges[0].UpdateDiffExpect {
		t.Fatalf("tokens must be unique per file change: %#v", view.Implementation)
	}

	implRaw := md[sectionSpans.Implementation.Start:sectionSpans.Implementation.End]
	if !strings.Contains(implRaw, "### 1. First") || !strings.Contains(implRaw, "### 2. Second") {
		t.Fatalf("implementation span missing expected step headings")
	}
}

// Wrapped frontmatter remains parser-owned: keep acceptance and rejection coverage here while JSON authoring surfaces are removed elsewhere.
func TestParseMarkdownAllowsLeadingFrontmatter(t *testing.T) {
	plan := Plan{
		Title:    "Plan",
		Overview: "Overview text.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []ChecklistItem{{Text: "One goal"}},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: []Step{{
			Title:   "First",
			Summary: "summary one",
			FileChanges: []FileChange{{
				Filename:    "a.txt",
				Explanation: "explain",
				Diff:        "@@ -1 +1 @@\n-old\n+new",
			}},
		}},
		Verification: &Verification{
			Summary:   "Verification summary.",
			Automated: []ChecklistItem{{Text: "go test ./..."}},
			Manual:    []ChecklistItem{{Text: "smoke"}},
		},
	}

	md, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	frontmatter := wrappedIssueFrontmatter()
	withFrontmatter := frontmatter + md

	result, err := ParseMarkdown(withFrontmatter)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	parsed := result.Plan
	sectionSpans := result.Sections
	if !reflect.DeepEqual(parsed, plan) {
		t.Fatalf("parsed plan mismatch:\nparsed=%#v\nwant=%#v", parsed, plan)
	}
	if sectionSpans.Overview.Start <= len(frontmatter) {
		t.Fatalf("overview span should be offset past frontmatter, got %d", sectionSpans.Overview.Start)
	}
	if !strings.Contains(withFrontmatter[sectionSpans.Overview.Start:sectionSpans.Overview.End], "Overview text.") {
		t.Fatal("overview span should point into original source with frontmatter")
	}
}

func TestTopicList(t *testing.T) {
	input := wrappedIssueFrontmatterWithTopics([]string{"planner", "cli", "parsing"}) + buildPlanNoFrontmatter(t)
	if _, err := ParseMarkdown(input); err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
}

func TestSplitMarkdownEnvelopePreservesCanonicalWrapperBytes(t *testing.T) {
	input := buildPlanWithFrontmatter(t)
	envelope, err := splitMarkdownEnvelope(input)
	if err != nil {
		t.Fatalf("splitMarkdownEnvelope: %v", err)
	}
	if !envelope.Wrapped {
		t.Fatal("expected wrapped envelope")
	}
	if envelope.Frontmatter != wrappedIssueFrontmatter() {
		t.Fatalf("frontmatter mismatch\nwant:\n%s\n\ngot:\n%s", wrappedIssueFrontmatter(), envelope.Frontmatter)
	}
	if envelope.BodyOffset != len(wrappedIssueFrontmatter()) {
		t.Fatalf("body offset = %d, want %d", envelope.BodyOffset, len(wrappedIssueFrontmatter()))
	}
	if !strings.HasPrefix(envelope.Body, "# Sample") {
		t.Fatalf("body not preserved: %q", envelope.Body)
	}
}

func TestSplitMarkdownEnvelopeRejectsBodyDividerClose(t *testing.T) {
	input := "---\n" +
		"tags:\n" +
		"  - \"#Ticket\"\n" +
		"type: issue\n" +
		"status: open\n" +
		"template_version: 1\n" +
		"project: PDEV-098\n" +
		"date_created: 2026-05-13\n" +
		"topics: []\n" +
		"# Wrapped Plan\n---\n\n" + buildPlanNoFrontmatter(t)
	_, err := splitMarkdownEnvelope(input)
	if !errors.Is(err, errUnterminatedWrappedDoc) {
		t.Fatalf("expected errUnterminatedWrappedDoc, got %v", err)
	}
}

func TestSplitMarkdownEnvelopeRejectsUnterminatedWrapper(t *testing.T) {
	input := strings.TrimSuffix(wrappedIssueFrontmatter(), "---\n\n") + buildPlanNoFrontmatter(t)
	_, err := splitMarkdownEnvelope(input)
	if !errors.Is(err, errUnterminatedWrappedDoc) {
		t.Fatalf("expected errUnterminatedWrappedDoc, got %v", err)
	}
}

func TestParseMarkdownTicketTagForms(t *testing.T) {
	cases := []struct {
		name    string
		replace string
		with    string
		wantErr bool
	}{
		{
			name:    "quoted ticket tag",
			replace: "  - \"#Ticket\"",
			with:    "  - \"#Ticket\"",
		},
		{
			name:    "unquoted ticket tag",
			replace: "  - \"#Ticket\"",
			with:    "  - #Ticket",
		},
		{
			name:    "multiple tags with ticket",
			replace: "type: issue",
			with:    "  - \"#Planner\"\ntype: issue",
		},
		{
			name:    "lowercase ticket tag",
			replace: "\"#Ticket\"",
			with:    "\"#ticket\"",
			wantErr: true,
		},
		{
			name:    "missing ticket tag",
			replace: "\"#Ticket\"",
			with:    "\"#Planner\"",
			wantErr: true,
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			input := strings.Replace(buildPlanWithFrontmatter(t), tc.replace, tc.with, 1)
			_, err := ParseMarkdown(input)
			if tc.wantErr {
				if err == nil {
					t.Fatal("expected unsupported frontmatter to fail")
				}
				if !strings.Contains(err.Error(), "unsupported wrapped issue doc frontmatter") {
					t.Fatalf("unexpected error: %v", err)
				}
				return
			}
			if err != nil {
				t.Fatalf("ParseMarkdown: %v", err)
			}
		})
	}
}

func TestParseMarkdownRejectsUnsupportedStatusValue(t *testing.T) {
	input := strings.Replace(buildPlanWithFrontmatter(t), "status: open", "status: closed", 1)
	if _, err := ParseMarkdown(input); err == nil {
		t.Fatal("expected unsupported frontmatter to fail")
	} else if !strings.Contains(err.Error(), "unsupported wrapped issue doc frontmatter") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestExecuteAcceptsWontDoIssue(t *testing.T) {
	input := strings.Replace(buildPlanWithFrontmatter(t), "status: open", "status: wont-do", 1)
	path := filepath.Join(t.TempDir(), "plan.md")
	if err := os.WriteFile(path, []byte(input), 0o600); err != nil {
		t.Fatal(err)
	}

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"inspect", path}, &stdout, &stderr); exit != 0 {
		t.Fatalf("Execute(inspect) exit=%d stderr=%q", exit, stderr.String())
	}
	var view InspectPlan
	if err := json.Unmarshal(stdout.Bytes(), &view); err != nil {
		t.Fatalf("inspect JSON: %v", err)
	}
	if view.Title == "" {
		t.Fatalf("empty inspect view: %+v", view)
	}
}

func TestParseMarkdownRejectsMalformedDateCreated(t *testing.T) {
	input := strings.Replace(buildPlanWithFrontmatter(t), "date_created: 2026-05-12", "date_created: 2026/05/12", 1)
	if _, err := ParseMarkdown(input); err == nil {
		t.Fatal("expected unsupported frontmatter to fail")
	} else if !strings.Contains(err.Error(), "unsupported wrapped issue doc frontmatter") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestParseMarkdownRejectsReorderedFields(t *testing.T) {
	input := "---\n" +
		"tags:\n" +
		"  - \"#Ticket\"\n" +
		"type: issue\n" +
		"template_version: 1\n" +
		"status: open\n" +
		"project: PDEV-083\n" +
		"date_created: 2026-05-12\n" +
		"topics: []\n" +
		"---\n\n" + buildPlanNoFrontmatter(t)
	if _, err := ParseMarkdown(input); err == nil {
		t.Fatal("expected reordered frontmatter to fail")
	} else if !strings.Contains(err.Error(), "unsupported wrapped issue doc frontmatter") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestParseMarkdownRejectsEmptyTopicsBlock(t *testing.T) {
	input := "---\n" +
		"tags:\n" +
		"  - \"#Ticket\"\n" +
		"type: issue\n" +
		"status: open\n" +
		"template_version: 1\n" +
		"project: PDEV-083\n" +
		"date_created: 2026-05-12\n" +
		"topics:\n" +
		"---\n\n" + buildPlanNoFrontmatter(t)
	if _, err := ParseMarkdown(input); err == nil {
		t.Fatal("expected empty topics block to fail")
	} else if !strings.Contains(err.Error(), "unsupported wrapped issue doc frontmatter") {
		t.Fatalf("unexpected error: %v", err)
	}
}

func TestParseMarkdownRejectsDuplicateScalarField(t *testing.T) {
	input := "---\n" +
		"tags:\n" +
		"  - \"#Ticket\"\n" +
		"type: issue\n" +
		"status: open\n" +
		"status: done\n" +
		"template_version: 1\n" +
		"project: PDEV-083\n" +
		"date_created: 2026-05-12\n" +
		"topics: []\n" +
		"---\n\n" + buildPlanNoFrontmatter(t)
	if _, err := ParseMarkdown(input); err == nil {
		t.Fatal("expected duplicate scalar field to fail")
	} else if !strings.Contains(err.Error(), "unsupported wrapped issue doc frontmatter") {
		t.Fatalf("unexpected error: %v", err)
	}
}

// TestParseMarkdownAllowsEmptyImplementationSection verifies that a plan rendered
// with no implementation steps can be parsed without error. This is the bootstrap
// case for append: an agent creates a plan skeleton and appends steps incrementally.
func TestParseMarkdownAllowsEmptyImplementationSection(t *testing.T) {
	plan := Plan{
		Title:    "Plan",
		Overview: "Overview text.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []ChecklistItem{{Text: "One goal"}},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: nil,
		Verification: &Verification{
			Summary:   "Verification summary.",
			Automated: []ChecklistItem{{Text: "go test ./..."}},
			Manual:    []ChecklistItem{{Text: "smoke"}},
		},
	}

	md, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}

	result, err := ParseMarkdown(md)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	parsed := result.Plan
	stepSpans := result.Steps
	if !reflect.DeepEqual(parsed, plan) {
		t.Fatalf("parsed plan mismatch:\nparsed=%#v\nwant=%#v", parsed, plan)
	}
	if len(stepSpans) != 0 {
		t.Fatalf("expected 0 step spans, got %d", len(stepSpans))
	}
}

func TestParseChecklistItemsRejectsMalformedMarker(t *testing.T) {
	_, err := parseChecklistItems("- [?] bad marker")
	if err == nil {
		t.Fatal("expected error for unrecognized marker")
	}
}

func TestParseMarkdownRejectsCRLF(t *testing.T) {
	_, err := ParseMarkdown("# Title\r\n## Overview\r\n")
	if err == nil || !strings.Contains(err.Error(), "CRLF") {
		t.Fatalf("expected CRLF error, got: %v", err)
	}
}

func TestSectionBodyRejectsDividerNotAfterHeading(t *testing.T) {
	input := "## Overview\nJunk before divider\n---\n\nBody"
	_, _, err := sectionBody(input, Span{Start: 0, End: len(input)})
	if err == nil {
		t.Fatal("expected error for misplaced divider")
	}
}

func TestSectionBodyAllowsThematicBreakInContent(t *testing.T) {
	input := "## Overview\n---\n\nText\n\n---\n\nMore after thematic break"
	body, _, err := sectionBody(input, Span{Start: 0, End: len(input)})
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if !strings.Contains(body, "thematic break") {
		t.Fatal("body should include content after thematic break")
	}
}

// TestRoundTripPreservesCheckboxStatus verifies that render -> inspect -> render
// produces byte-identical output and preserves mixed unchecked/done checkbox
// states across the full cycle.
func TestRoundTripPreservesCheckboxStatus(t *testing.T) {
	plan := Plan{
		Title:    "Round-trip",
		Overview: "Overview.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []ChecklistItem{{Text: "Pending goal"}, {Text: "Done goal", Status: StatusDone}},
			CurrentState: "Current.",
			ModuleShape:  "Shape.",
		},
		Implementation: []Step{{
			Title:       "Step",
			Summary:     "summary",
			FileChanges: []FileChange{{Filename: "f.go", Explanation: "why", Diff: "@@ -1 +1 @@\n-a\n+b"}},
		}},
		Verification: &Verification{
			Summary:   "",
			Automated: []ChecklistItem{{Text: "auto"}, {Text: "done auto", Status: StatusDone}},
			Manual:    []ChecklistItem{{Text: "manual"}},
		},
	}

	md, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}

	result, err := ParseMarkdown(md)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	if !reflect.DeepEqual(result.Plan, plan) {
		t.Fatalf("parsed plan mismatch:\nparsed=%#v\nwant=%#v", result.Plan, plan)
	}

	rerendered, err := RenderPlan(result.Plan)
	if err != nil {
		t.Fatalf("RenderPlan (re-render): %v", err)
	}
	if md != rerendered {
		t.Fatalf("re-render not byte-identical:\nfirst=%q\nsecond=%q", md, rerendered)
	}
}

func TestParseMarkdownReturnsDiffContentSpans(t *testing.T) {
	plan := Plan{
		Title:    "Plan",
		Overview: "Overview text.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []ChecklistItem{{Text: "One goal"}},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: []Step{
			{
				Title:   "First",
				Summary: "summary one",
				FileChanges: []FileChange{
					{
						Filename:    "a.txt",
						Explanation: "explain",
						Diff:        "@@ -1 +1 @@\n-old\n+new",
					},
					{
						Filename:    "b.txt",
						Explanation: "explain",
						Diff:        "@@ -1 +1 @@\n-old\n+newer",
					},
				},
			},
		},
		Verification: &Verification{
			Summary:   "Verification summary.",
			Automated: []ChecklistItem{{Text: "go test ./..."}},
			Manual:    []ChecklistItem{{Text: "smoke"}},
		},
	}

	md, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}

	result, err := ParseMarkdown(md)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	diffSpans := result.DiffContents
	if len(diffSpans) != len(plan.Implementation) {
		t.Fatalf("expected %d step span rows, got %d", len(plan.Implementation), len(diffSpans))
	}
	if len(diffSpans[0]) != len(plan.Implementation[0].FileChanges) {
		t.Fatalf("expected %d file-change spans, got %d", len(plan.Implementation[0].FileChanges), len(diffSpans[0]))
	}
	for i, span := range diffSpans[0] {
		if span.Start < 0 || span.End <= span.Start {
			t.Fatalf("span %d malformed: %+v", i, span)
		}
		got := md[span.Start:span.End]
		want := plan.Implementation[0].FileChanges[i].Diff
		if !strings.Contains(got, want) {
			t.Fatalf("span %d missing diff content: got=%q want=%q", i, got, want)
		}
	}
}

func TestParseMarkdownReturnsTitleSpan(t *testing.T) {
	for _, tc := range []struct {
		name  string
		build func(*testing.T) string
	}{
		{name: "no frontmatter", build: buildPlanNoFrontmatter},
		{name: "with frontmatter", build: buildPlanWithFrontmatter},
	} {
		t.Run(tc.name, func(t *testing.T) {
			input := tc.build(t)
			result, err := ParseMarkdown(input)
			if err != nil {
				t.Fatalf("ParseMarkdown: %v", err)
			}
			got := input[result.Sections.Title.Start:result.Sections.Title.End]
			if got != result.Plan.Title {
				t.Fatalf("title span = %q, want %q", got, result.Plan.Title)
			}
		})
	}
}

func TestParseMarkdownRejectsBadFilenameShapes(t *testing.T) {
	md, err := RenderPlan(Plan{
		Title:    "Plan",
		Overview: "Overview text.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []ChecklistItem{{Text: "One goal"}},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: []Step{{
			Title:   "First",
			Summary: "summary one",
			FileChanges: []FileChange{{
				Filename:    "a.txt",
				Explanation: "explain",
				Diff:        "@@ -1 +1 @@\n-old\n+new",
			}},
		}},
		Verification: &Verification{
			Summary:   "Verification summary.",
			Automated: []ChecklistItem{{Text: "go test ./..."}},
			Manual:    []ChecklistItem{{Text: "smoke"}},
		},
	})
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}

	for _, tc := range []struct {
		name        string
		replacement string
		wantSubstr  string
	}{
		{
			name:        "contains whitespace",
			replacement: "`not a file`",
			wantSubstr:  "contains whitespace",
		},
		{
			name:        "not path shaped",
			replacement: "`<path/to/file>`",
			wantSubstr:  "not a path-shape",
		},
		{
			name:        "empty after trim",
			replacement: "`   `",
			wantSubstr:  "empty after trim",
		},
	} {
		t.Run(tc.name, func(t *testing.T) {
			bad := strings.Replace(md, "`a.txt`", tc.replacement, 1)
			_, err := ParseMarkdown(bad)
			if err == nil {
				t.Fatal("expected parse error")
			}
			if !strings.Contains(err.Error(), tc.wantSubstr) {
				t.Fatalf("error %q does not contain %q", err.Error(), tc.wantSubstr)
			}
		})
	}
}

func TestParseMarkdownKeepsInlineCodeSummary(t *testing.T) {
	md := buildPlanNoFrontmatter(t)
	// Inline code at the start of a wrapped summary line must not be
	// parsed as a filename.
	md = strings.Replace(md, "s\n\n`a.go`", "...stay\n`provider-unavailable`.\n\n`a.go`", 1)

	result, err := ParseMarkdown(md)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	if got, want := result.Plan.Implementation[0].Summary, "...stay\n`provider-unavailable`."; got != want {
		t.Fatalf("summary=%q want %q", got, want)
	}
	changes := result.Plan.Implementation[0].FileChanges
	if len(changes) != 1 || changes[0].Filename != "a.go" {
		t.Fatalf("file changes=%+v, want one change for a.go", changes)
	}
}

func TestParseMarkdownRejectsInvalidStructuredFilename(t *testing.T) {
	md := strings.Replace(buildPlanNoFrontmatter(t), "`a.go`", "`bad path`", 1)

	_, err := ParseMarkdown(md)
	if err == nil || !strings.Contains(err.Error(), "invalid file change filename") || !strings.Contains(err.Error(), "contains whitespace") {
		t.Fatalf("expected clear invalid filename error, got %v", err)
	}
}

func TestParseMarkdownRejectsMissingExplanation(t *testing.T) {
	result, err := ParseMarkdown(buildPlanNoFrontmatter(t))
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	plan := result.Plan
	plan.Implementation[0].FileChanges = append(plan.Implementation[0].FileChanges, FileChange{
		Filename:    "b.go",
		Explanation: "second change",
		Diff:        "@@ -1 +1 @@\n-old\n+new",
	})
	md, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	md = strings.Replace(md, "`b.go`\n> second change\n\n```diff", "`b.go`\n\n```diff", 1)
	if !strings.Contains(md, "`b.go`\n\n```diff") {
		t.Fatal("test input is missing the second header's explanation")
	}
	headerOffset := strings.Index(md, "`b.go`")
	if headerOffset < 0 {
		t.Fatal("test input is missing the second header")
	}
	wantLine := strings.Count(md[:headerOffset], "\n") + 1

	_, err = ParseMarkdown(md)
	wantError := fmt.Sprintf("malformed file change header at line %d:", wantLine)
	if err == nil || !strings.Contains(err.Error(), wantError) {
		t.Fatalf("error=%v, want line-specific error containing %q", err, wantError)
	}
}

func buildPlanNoFrontmatter(t *testing.T) string {
	t.Helper()
	plan := Plan{
		Title:    "Sample",
		Overview: "o",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "n",
			Goals:        []ChecklistItem{{Text: "g"}},
			CurrentState: "c",
			ModuleShape:  "m",
		},
		Implementation: []Step{{
			Title:   "t",
			Summary: "s",
			FileChanges: []FileChange{{
				Filename:    "a.go",
				Explanation: "e",
				Diff:        "@@ -1 +1 @@\n-x\n+y",
			}},
		}},
		Verification: &Verification{
			Summary:   "vs",
			Automated: []ChecklistItem{{Text: "a"}},
			Manual:    []ChecklistItem{{Text: "m"}},
		},
	}
	out, err := RenderPlan(plan)
	if err != nil {
		t.Fatal(err)
	}
	return out
}

func wrappedIssueFrontmatter() string {
	return wrappedIssueFrontmatterWithTopics(nil)
}

func wrappedIssueFrontmatterWithTopics(topics []string) string {
	var b strings.Builder
	b.WriteString("---\n")
	b.WriteString("tags:\n")
	b.WriteString("  - \"#Ticket\"\n")
	b.WriteString("type: issue\n")
	b.WriteString("status: open\n")
	b.WriteString("template_version: 1\n")
	b.WriteString("project: PDEV-083\n")
	b.WriteString("date_created: 2026-05-12\n")
	if len(topics) == 0 {
		b.WriteString("topics: []\n")
	} else {
		b.WriteString("topics:\n")
		for _, topic := range topics {
			b.WriteString("  - ")
			b.WriteString(topic)
			b.WriteString("\n")
		}
	}
	b.WriteString("---\n\n")
	return b.String()
}

func buildPlanWithFrontmatter(t *testing.T) string {
	t.Helper()
	return wrappedIssueFrontmatter() + buildPlanNoFrontmatter(t)
}
