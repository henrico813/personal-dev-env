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
			Goals:        []string{"One goal"},
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
			Automated: []string{"go test ./..."},
			Manual:    []string{"smoke"},
		},
	}

	md, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}

	parsed, sectionSpans, stepSpans, err := ParseMarkdown(md)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
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
			Goals:        []string{"One goal"},
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
			Automated: []string{"go test ./..."},
			Manual:    []string{"smoke"},
		},
	}

	md, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	frontmatter := "---\ntags:\n  - \"#Ticket\"\n---\n\n"
	withFrontmatter := frontmatter + md

	parsed, sectionSpans, _, err := ParseMarkdown(withFrontmatter)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
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
			Goals:        []string{"One goal"},
			CurrentState: "Current state.",
			ModuleShape:  "module shape",
		},
		Implementation: nil,
		Verification: &schema.Verification{
			Summary:   "Verification summary.",
			Automated: []string{"go test ./..."},
			Manual:    []string{"smoke"},
		},
	}

	md, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}

	parsed, _, stepSpans, err := ParseMarkdown(md)
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	if !reflect.DeepEqual(parsed, plan) {
		t.Fatalf("parsed plan mismatch:\nparsed=%#v\nwant=%#v", parsed, plan)
	}
	if len(stepSpans) != 0 {
		t.Fatalf("expected 0 step spans, got %d", len(stepSpans))
	}
}

func TestParseMarkdownRejectsCRLF(t *testing.T) {
	_, _, _, err := ParseMarkdown("# Title\r\n## Overview\r\n")
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
