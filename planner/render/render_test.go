package render

import (
	"strings"
	"testing"

	"planner/schema"
)

func TestRenderPlanEmitsUncheckedForPendingOrEmptyStatus(t *testing.T) {
	plan := minimalPlan()
	plan.DefinitionOfDone.Goals = []schema.ChecklistItem{{Text: "pending goal"}}
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
	plan.DefinitionOfDone.Goals = []schema.ChecklistItem{{Text: "done goal", Status: schema.StatusDone}}
	out, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	if !strings.Contains(out, "- [x] done goal") {
		t.Fatalf("expected checked render, got:\n%s", out)
	}
}

func TestRenderPlanFromExampleDoesNotError(t *testing.T) {
	if _, err := RenderPlan(schema.BuildPlanExample()); err != nil {
		t.Fatalf("BuildPlanExample should render cleanly: %v", err)
	}
}

func minimalPlan() schema.Plan {
	return schema.Plan{
		Title:    "T",
		Overview: "Overview text.",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "Narrative.",
			Goals:        []schema.ChecklistItem{{Text: "g"}},
			CurrentState: "Current.",
			ModuleShape:  "Shape.",
		},
		Implementation: []schema.Step{{
			Title:   "Step",
			Summary: "summary",
			FileChanges: []schema.FileChange{{
				Filename:    "f.go",
				Explanation: "why",
				Diff:        "@@ -1 +1 @@\n-a\n+b",
			}},
		}},
		Verification: &schema.Verification{
			Summary:   "",
			Automated: []schema.ChecklistItem{{Text: "a"}},
			Manual:    []schema.ChecklistItem{{Text: "m"}},
		},
	}
}
