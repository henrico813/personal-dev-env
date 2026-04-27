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
			Goals:        []schema.ChecklistItem{{Text: "Rendered plan goals follow the configured limits."}},
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
	plan.DefinitionOfDone.Goals = make([]schema.ChecklistItem, maxDefinitionOfDoneGoals)
	for i := range plan.DefinitionOfDone.Goals {
		plan.DefinitionOfDone.Goals[i] = schema.ChecklistItem{Text: strings.Repeat("a", maxDefinitionOfDoneGoalLength)}
	}

	if err := ValidatePlan(plan); err != nil {
		t.Fatalf("ValidatePlan() error = %v, want nil", err)
	}
}

func TestValidatePlanRejectsTooManyGoals(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = make([]schema.ChecklistItem, maxDefinitionOfDoneGoals+1)
	for i := range plan.DefinitionOfDone.Goals {
		plan.DefinitionOfDone.Goals[i] = schema.ChecklistItem{Text: "short goal"}
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
	plan.DefinitionOfDone.Goals = []schema.ChecklistItem{{Text: strings.Repeat("a", maxDefinitionOfDoneGoalLength+1)}}

	err := ValidatePlan(plan)
	if err == nil {
		t.Fatal("ValidatePlan() error = nil, want error")
	}
	if got, want := err.Error(), "definition_of_done.goals[0] must be no more than 88 characters"; got != want {
		t.Fatalf("ValidatePlan() error = %q, want %q", got, want)
	}
}

func TestValidatePlanRejectsInvalidGoalStatus(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = []schema.ChecklistItem{{Text: "goal", Status: "started"}}
	if err := ValidatePlan(plan); err == nil {
		t.Fatal("expected error for invalid goal status")
	}
}

func TestValidatePlanRejectsEmptyGoalText(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = []schema.ChecklistItem{{Text: "  "}}
	if err := ValidatePlan(plan); err == nil {
		t.Fatal("expected error for empty goal text")
	}
}

func TestValidatePlanRejectsEmptyVerificationItemText(t *testing.T) {
	plan := validPlan()
	plan.Verification = &schema.Verification{
		Automated: []schema.ChecklistItem{{Text: ""}},
		Manual:    []schema.ChecklistItem{{Text: "m"}},
	}
	if err := ValidatePlan(plan); err == nil {
		t.Fatal("expected error for empty automated text")
	}
}

func TestValidatePlanRejectsInvalidVerificationItemStatus(t *testing.T) {
	plan := validPlan()
	plan.Verification = &schema.Verification{
		Automated: []schema.ChecklistItem{{Text: "a", Status: "invalid"}},
		Manual:    []schema.ChecklistItem{{Text: "m"}},
	}
	if err := ValidatePlan(plan); err == nil {
		t.Fatal("expected error for invalid automated status")
	}
}

func TestValidatePlanFilenameShapeMatrix(t *testing.T) {
	tests := []struct {
		name       string
		filename   string
		wantSubstr string
		wantErr    bool
	}{
		{
			name:     "accepts path shape",
			filename: "planner/validate/validate.go",
			wantErr:  false,
		},
		{
			name:       "rejects whitespace",
			filename:   "path/with space.go",
			wantSubstr: "contains whitespace",
			wantErr:    true,
		},
		{
			name:       "rejects angle-bracket placeholder",
			filename:   "<path/to/file>",
			wantSubstr: "not a path-shape",
			wantErr:    true,
		},
		{
			name:       "rejects empty after trim",
			filename:   "   ",
			wantSubstr: "empty after trim",
			wantErr:    true,
		},
		{
			name:       "rejects long filename",
			filename:   strings.Repeat("a", 201),
			wantSubstr: "200-byte limit",
			wantErr:    true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			plan := validPlan()
			plan.Implementation[0].FileChanges[0].Filename = tc.filename

			err := ValidatePlan(plan)
			if tc.wantErr {
				if err == nil {
					t.Fatal("ValidatePlan() error = nil, want error")
				}
				if !strings.Contains(err.Error(), tc.wantSubstr) {
					t.Fatalf("ValidatePlan() error = %q, want substring %q", err.Error(), tc.wantSubstr)
				}
				return
			}
			if err != nil {
				t.Fatalf("ValidatePlan() error = %v, want nil", err)
			}
		})
	}
}
