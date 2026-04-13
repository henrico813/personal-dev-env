package validate

import (
	"strings"
	"testing"

	"planner/schema"
)

func validPlan() schema.Plan {
	return schema.Plan{
		Title:    "Planner validation rules",
		Overview: "Add goal-count and goal-length validation for rendered plans.",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "Goals should stay short and capped so the rendered markdown stays scannable.",
			Goals:        []string{"Rendered plan goals follow the configured limits."},
			CurrentState: "The planner currently requires at least one goal but does not cap count or length.",
			ModuleShape:  "Validation remains centralized in planner/validate.",
		},
		Implementation: []schema.Step{
			{
				Title:   "Validate goals",
				Summary: "Reject plans with too many goals or goals that are too long.",
				FileChanges: []schema.FileChange{
					{
						Filename:    "planner/validate/validate.go",
						Explanation: "Adds the markdown goal constraints.",
						Diff:        "@@ -1 +1 @@\n- old\n+ new",
					},
				},
			},
		},
		Verification: &schema.Verification{},
	}
}

func TestValidatePlanAcceptsGoalLimits(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = make([]string, maxDefinitionOfDoneGoals)
	for i := range plan.DefinitionOfDone.Goals {
		plan.DefinitionOfDone.Goals[i] = strings.Repeat("a", maxDefinitionOfDoneGoalLength)
	}

	if err := ValidatePlan(plan); err != nil {
		t.Fatalf("ValidatePlan() error = %v, want nil", err)
	}
}

func TestValidatePlanRejectsTooManyGoals(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = make([]string, maxDefinitionOfDoneGoals+1)
	for i := range plan.DefinitionOfDone.Goals {
		plan.DefinitionOfDone.Goals[i] = "short goal"
	}

	err := ValidatePlan(plan)
	if err == nil {
		t.Fatal("ValidatePlan() error = nil, want error")
	}
	if got, want := err.Error(), "definition_of_done.goals must have no more than 8 goals"; got != want {
		t.Fatalf("ValidatePlan() error = %q, want %q", got, want)
	}
}

func TestValidatePlanRejectsGoalLongerThanLimit(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = []string{strings.Repeat("a", maxDefinitionOfDoneGoalLength+1)}

	err := ValidatePlan(plan)
	if err == nil {
		t.Fatal("ValidatePlan() error = nil, want error")
	}
	if got, want := err.Error(), "definition_of_done.goals[0] must be no more than 88 characters"; got != want {
		t.Fatalf("ValidatePlan() error = %q, want %q", got, want)
	}
}
