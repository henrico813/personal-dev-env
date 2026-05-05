package internal

import (
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
