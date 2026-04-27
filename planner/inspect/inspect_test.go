package inspect

import (
	"reflect"
	"strings"
	"testing"

	"planner/render"
	"planner/schema"
)

func TestParseMarkdownRoundTripFromRenderPlan(t *testing.T) {
	plan := schema.Plan{
		Title:    "Plan",
		Overview: "Overview text.",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []schema.ChecklistItem{{Text: "One goal"}},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: []schema.Step{
			{
				Title:   "First",
				Summary: "summary one",
				FileChanges: []schema.FileChange{{
					Filename:    "a.txt",
					Explanation: "explain",
					Diff:        "@@ -1 +1 @@\n-old\n+new",
				}},
			},
			{
				Title:   "Second",
				Summary: "summary two",
				FileChanges: []schema.FileChange{{
					Filename:    "b.txt",
					Explanation: "explain",
					Diff:        "@@ -1 +1 @@\n-old\n+new",
				}},
			},
		},
		Verification: &schema.Verification{
			Summary:   "Verification summary.",
			Automated: []schema.ChecklistItem{{Text: "go test ./..."}},
			Manual:    []schema.ChecklistItem{{Text: "smoke"}},
		},
	}

	md, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}

	result, err := ParseMarkdown(md)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	parsed := result.Plan
	sectionSpans := result.Sections
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

	implRaw := md[sectionSpans.Implementation.Start:sectionSpans.Implementation.End]
	if !strings.Contains(implRaw, "### 1. First") || !strings.Contains(implRaw, "### 2. Second") {
		t.Fatalf("implementation span missing expected step headings")
	}
}

// TestParseMarkdownAllowsLeadingFrontmatter verifies that vault-style issue files
// (which prepend YAML frontmatter before the plan title) are parsed correctly and
// that returned spans are absolute into the original source (including frontmatter).
func TestParseMarkdownAllowsLeadingFrontmatter(t *testing.T) {
	plan := schema.Plan{
		Title:    "Plan",
		Overview: "Overview text.",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []schema.ChecklistItem{{Text: "One goal"}},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: []schema.Step{{
			Title:   "First",
			Summary: "summary one",
			FileChanges: []schema.FileChange{{
				Filename:    "a.txt",
				Explanation: "explain",
				Diff:        "@@ -1 +1 @@\n-old\n+new",
			}},
		}},
		Verification: &schema.Verification{
			Summary:   "Verification summary.",
			Automated: []schema.ChecklistItem{{Text: "go test ./..."}},
			Manual:    []schema.ChecklistItem{{Text: "smoke"}},
		},
	}

	md, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	frontmatter := "---\ntags:\n  - \"#Ticket\"\n---\n\n"
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

// TestParseMarkdownAllowsEmptyImplementationSection verifies that a plan rendered
// with no implementation steps can be parsed without error. This is the bootstrap
// case for append: an agent creates a plan skeleton and appends steps incrementally.
func TestParseMarkdownAllowsEmptyImplementationSection(t *testing.T) {
	plan := schema.Plan{
		Title:    "Plan",
		Overview: "Overview text.",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []schema.ChecklistItem{{Text: "One goal"}},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: nil,
		Verification: &schema.Verification{
			Summary:   "Verification summary.",
			Automated: []schema.ChecklistItem{{Text: "go test ./..."}},
			Manual:    []schema.ChecklistItem{{Text: "smoke"}},
		},
	}

	md, err := render.RenderPlan(plan)
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
	plan := schema.Plan{
		Title:    "Round-trip",
		Overview: "Overview.",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []schema.ChecklistItem{{Text: "Pending goal"}, {Text: "Done goal", Status: schema.StatusDone}},
			CurrentState: "Current.",
			ModuleShape:  "Shape.",
		},
		Implementation: []schema.Step{{
			Title:       "Step",
			Summary:     "summary",
			FileChanges: []schema.FileChange{{Filename: "f.go", Explanation: "why", Diff: "@@ -1 +1 @@\n-a\n+b"}},
		}},
		Verification: &schema.Verification{
			Summary:   "",
			Automated: []schema.ChecklistItem{{Text: "auto"}, {Text: "done auto", Status: schema.StatusDone}},
			Manual:    []schema.ChecklistItem{{Text: "manual"}},
		},
	}

	md, err := render.RenderPlan(plan)
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

	rerendered, err := render.RenderPlan(result.Plan)
	if err != nil {
		t.Fatalf("RenderPlan (re-render): %v", err)
	}
	if md != rerendered {
		t.Fatalf("re-render not byte-identical:\nfirst=%q\nsecond=%q", md, rerendered)
	}
}

func TestParseMarkdownReturnsDiffContentSpans(t *testing.T) {
	plan := schema.Plan{
		Title:    "Plan",
		Overview: "Overview text.",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []schema.ChecklistItem{{Text: "One goal"}},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: []schema.Step{
			{
				Title:   "First",
				Summary: "summary one",
				FileChanges: []schema.FileChange{
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
		Verification: &schema.Verification{
			Summary:   "Verification summary.",
			Automated: []schema.ChecklistItem{{Text: "go test ./..."}},
			Manual:    []schema.ChecklistItem{{Text: "smoke"}},
		},
	}

	md, err := render.RenderPlan(plan)
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
	md, err := render.RenderPlan(schema.Plan{
		Title:    "Plan",
		Overview: "Overview text.",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []schema.ChecklistItem{{Text: "One goal"}},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: []schema.Step{{
			Title:   "First",
			Summary: "summary one",
			FileChanges: []schema.FileChange{{
				Filename:    "a.txt",
				Explanation: "explain",
				Diff:        "@@ -1 +1 @@\n-old\n+new",
			}},
		}},
		Verification: &schema.Verification{
			Summary:   "Verification summary.",
			Automated: []schema.ChecklistItem{{Text: "go test ./..."}},
			Manual:    []schema.ChecklistItem{{Text: "smoke"}},
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

func buildPlanNoFrontmatter(t *testing.T) string {
	t.Helper()
	plan := schema.Plan{
		Title:    "Sample",
		Overview: "o",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "n",
			Goals:        []schema.ChecklistItem{{Text: "g"}},
			CurrentState: "c",
			ModuleShape:  "m",
		},
		Implementation: []schema.Step{{
			Title:   "t",
			Summary: "s",
			FileChanges: []schema.FileChange{{
				Filename:    "a.go",
				Explanation: "e",
				Diff:        "@@ -1 +1 @@\n-x\n+y",
			}},
		}},
		Verification: &schema.Verification{
			Summary:   "vs",
			Automated: []schema.ChecklistItem{{Text: "a"}},
			Manual:    []schema.ChecklistItem{{Text: "m"}},
		},
	}
	out, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatal(err)
	}
	return out
}

func buildPlanWithFrontmatter(t *testing.T) string {
	t.Helper()
	return "---\nproject: DevEnv\ndate_created: 2026-04-26\n---\n" + buildPlanNoFrontmatter(t)
}
