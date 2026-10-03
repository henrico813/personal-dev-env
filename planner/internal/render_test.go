package internal

import (
	"strings"
	"testing"
	"time"
)

func TestRenderCanonicalScaffoldPassesValidation(t *testing.T) {
	rendered, err := renderCanonicalScaffold()
	if err != nil {
		t.Fatalf("renderCanonicalScaffold: %v", err)
	}
	if _, err := ParseMarkdown(rendered); err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
}

// The issue scaffold must start with the exact vault block the parser accepts
// and keep the default scaffold body after it. Drift here would make every
// vault plan fail its first check.
func TestRenderIssueScaffoldMatchesVaultFrontmatter(t *testing.T) {
	date := time.Date(2026, 10, 2, 0, 0, 0, 0, time.UTC)
	got, err := renderIssueScaffold("DevEnv", date)
	if err != nil {
		t.Fatalf("renderIssueScaffold: %v", err)
	}
	wantFrontmatter := "---\ntags:\n  - \"#Ticket\"\ntype: issue\nstatus: open\ntemplate_version: 1\nproject: DevEnv\ndate_created: 2026-10-02\ntopics: []\n---\n\n"
	if !strings.HasPrefix(got, wantFrontmatter) {
		t.Fatalf("frontmatter prefix mismatch:\n%s", got)
	}
	body, err := renderCanonicalScaffold()
	if err != nil {
		t.Fatal(err)
	}
	if got != wantFrontmatter+body {
		t.Fatal("issue scaffold changed the default body")
	}
}

func TestRenderPlanEmitsUncheckedForPendingOrEmptyStatus(t *testing.T) {
	plan := minimalPlan()
	plan.DefinitionOfDone.Goals = []ChecklistItem{{Text: "pending goal"}}
	out, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	if !strings.Contains(out, "- [ ] pending goal") {
		t.Fatalf("expected unchecked render, got:\n%s", out)
	}
}

func TestRenderPlanEmitsCheckedForStatusDone(t *testing.T) {
	plan := minimalPlan()
	plan.DefinitionOfDone.Goals = []ChecklistItem{{Text: "done goal", Status: StatusDone}}
	out, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	if !strings.Contains(out, "- [x] done goal") {
		t.Fatalf("expected checked render, got:\n%s", out)
	}
}

func TestRenderPlanFromExampleDoesNotError(t *testing.T) {
	if _, err := RenderPlan(BuildPlanExample()); err != nil {
		t.Fatalf("BuildPlanExample should render cleanly: %v", err)
	}
}

func minimalPlan() Plan {
	return Plan{
		Title:    "T",
		Overview: "Overview text.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []ChecklistItem{{Text: "g"}},
			CurrentState: "Current.",
			ModuleShape:  "Shape.",
		},
		Implementation: []Step{{
			Title:   "Step",
			Summary: "summary",
			FileChanges: []FileChange{{
				Filename:    "f.go",
				Explanation: "why",
				Diff:        "@@ -1 +1 @@\n-a\n+b",
			}},
		}},
		Verification: &Verification{
			Summary:   "",
			Automated: []ChecklistItem{{Text: "a"}},
			Manual:    []ChecklistItem{{Text: "m"}},
		},
	}
}
