package internal

import (
	"os"
	"strings"
	"testing"
)

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

func TestCreatePlanFromStructPreservesExistingFrontmatter(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/plan.md"
	frontmatter := "---\ntags:\n  - \"#Ticket\"\ntype: issue\ntemplate_version: 1\ntopics: []\nstatus: open\nproject: PDEV-083\ndate_created: 2026-05-12\n---\n\n"
	if err := os.WriteFile(out, []byte(frontmatter+"old body\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	plan := minimalPlan()
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	if err := CreatePlanFromStruct(plan, out); err != nil {
		t.Fatalf("CreatePlanFromStruct: %v", err)
	}
	raw, err := os.ReadFile(out)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	if got, want := string(raw), frontmatter+rendered; got != want {
		t.Fatalf("rewritten output mismatch\nwant:\n%s\n\ngot:\n%s", want, got)
	}
}

func TestCreatePlanFromStructLeavesNewOutputsUnchanged(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/plan.md"

	plan := minimalPlan()
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	if err := CreatePlanFromStruct(plan, out); err != nil {
		t.Fatalf("CreatePlanFromStruct: %v", err)
	}
	raw, err := os.ReadFile(out)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	if got, want := string(raw), rendered; got != want {
		t.Fatalf("new output mismatch\nwant:\n%s\n\ngot:\n%s", want, got)
	}
}

func TestCreatePlanFromStructRejectsUnsupportedFrontmatter(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/plan.md"
	if err := os.WriteFile(out, []byte("---\ntags:\n  - \"#ticket\"\ntype: issue\ntemplate_version: 1\ntopics: []\nstatus: open\nproject: PDEV-083\ndate_created: 2026-05-12\n---\n\nold body\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	if err := CreatePlanFromStruct(minimalPlan(), out); err == nil {
		t.Fatal("expected unsupported frontmatter to fail")
	} else if !strings.Contains(err.Error(), "unsupported frontmatter format") {
		t.Fatalf("unexpected error: %v", err)
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
