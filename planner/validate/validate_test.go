package validate

import (
	"fmt"
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
	if got, want := err.Error(), "definition_of_done.goals must have no more than 8 goals (got 9)"; got != want {
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
	if got, want := err.Error(), "definition_of_done.goals[0] must be no more than 88 characters (got 89)"; got != want {
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

func TestValidatePlanLengthMessagesIncludeActual(t *testing.T) {
	t.Run("goals_count", func(t *testing.T) {
		plan := validPlan()
		plan.DefinitionOfDone.Goals = make([]schema.ChecklistItem, maxDefinitionOfDoneGoals+2)
		for i := range plan.DefinitionOfDone.Goals {
			plan.DefinitionOfDone.Goals[i] = schema.ChecklistItem{Text: fmt.Sprintf("goal-%d", i)}
		}
		err := ValidatePlan(plan)
		if err == nil {
			t.Fatal("expected error")
		}
		if !strings.Contains(err.Error(), "(got 10)") {
			t.Fatalf("error %q does not contain actual count", err.Error())
		}
	})

	t.Run("goal_length", func(t *testing.T) {
		plan := validPlan()
		plan.DefinitionOfDone.Goals = []schema.ChecklistItem{{Text: strings.Repeat("a", maxDefinitionOfDoneGoalLength+1)}}
		err := ValidatePlan(plan)
		if err == nil {
			t.Fatal("expected error")
		}
		if !strings.Contains(err.Error(), "(got 89)") {
			t.Fatalf("error %q does not contain actual length", err.Error())
		}
	})
}

func TestValidatePlanRejectsNewLengthCaps(t *testing.T) {
	tests := []struct {
		name       string
		mutate     func(*schema.Plan)
		wantSubstr string
	}{
		{
			name: "title",
			mutate: func(p *schema.Plan) {
				p.Title = strings.Repeat("t", maxTitleLength+1)
			},
			wantSubstr: "title must be no more than 88 characters",
		},
		{
			name: "overview",
			mutate: func(p *schema.Plan) {
				p.Overview = strings.Repeat("o", maxOverviewLength+1)
			},
			wantSubstr: "overview must be no more than 500 characters",
		},
		{
			name: "narrative",
			mutate: func(p *schema.Plan) {
				p.DefinitionOfDone.Narrative = strings.Repeat("n", maxDoDNarrativeLength+1)
			},
			wantSubstr: "definition_of_done.narrative must be no more than 500 characters",
		},
		{
			name: "current_state",
			mutate: func(p *schema.Plan) {
				p.DefinitionOfDone.CurrentState = strings.Repeat("c", maxCurrentStateLength+1)
			},
			wantSubstr: "definition_of_done.current_state must be no more than 500 characters",
		},
		{
			name: "module_shape_line",
			mutate: func(p *schema.Plan) {
				p.DefinitionOfDone.ModuleShape = strings.Repeat("m", maxModuleShapeLineLength+1)
			},
			wantSubstr: "definition_of_done.module_shape line 1 must be no more than 88 characters",
		},
		{
			name: "step_title",
			mutate: func(p *schema.Plan) {
				p.Implementation[0].Title = strings.Repeat("s", maxStepTitleLength+1)
			},
			wantSubstr: "implementation[0].title must be no more than 88 characters",
		},
		{
			name: "step_summary",
			mutate: func(p *schema.Plan) {
				p.Implementation[0].Summary = strings.Repeat("s", maxStepSummaryLength+1)
			},
			wantSubstr: "implementation[0].summary must be no more than 500 characters",
		},
		{
			name: "file_change_explanation",
			mutate: func(p *schema.Plan) {
				p.Implementation[0].FileChanges[0].Explanation = strings.Repeat("e", maxFileChangeExplanationLength+1)
			},
			wantSubstr: "file_changes[0].explanation must be no more than 250 characters",
		},
		{
			name: "verification_automated_text",
			mutate: func(p *schema.Plan) {
				p.Verification.Automated = []schema.ChecklistItem{{Text: strings.Repeat("a", maxVerificationItemTextLength+1)}}
			},
			wantSubstr: "verification.automated[0].text must be no more than 88 characters",
		},
		{
			name: "verification_manual_text",
			mutate: func(p *schema.Plan) {
				p.Verification.Manual = []schema.ChecklistItem{{Text: strings.Repeat("m", maxVerificationItemTextLength+1)}}
			},
			wantSubstr: "verification.manual[0].text must be no more than 88 characters",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			plan := validPlan()
			tc.mutate(&plan)
			err := ValidatePlan(plan)
			if err == nil {
				t.Fatalf("expected error containing %q", tc.wantSubstr)
			}
			if !strings.Contains(err.Error(), tc.wantSubstr) {
				t.Fatalf("error %q does not contain %q", err.Error(), tc.wantSubstr)
			}
		})
	}
}

func TestValidatePlanAllReportsAllViolations(t *testing.T) {
	plan := validPlan()
	plan.Title = ""
	plan.Overview = strings.Repeat("o", maxOverviewLength+1)
	plan.DefinitionOfDone.Narrative = strings.Repeat("n", maxDoDNarrativeLength+1)
	plan.DefinitionOfDone.CurrentState = strings.Repeat("c", maxCurrentStateLength+1)
	plan.DefinitionOfDone.ModuleShape = strings.Repeat("m", maxModuleShapeLineLength+1)
	plan.Verification.Automated = []schema.ChecklistItem{{Text: strings.Repeat("a", maxVerificationItemTextLength+1)}}

	errs := ValidatePlanAll(plan)
	if len(errs) < 5 {
		t.Fatalf("expected at least 5 violations, got %d: %+v", len(errs), errs)
	}

	want := []string{
		"title is required",
		"overview must be no more than 500 characters",
		"definition_of_done.narrative must be no more than 500 characters",
		"definition_of_done.current_state must be no more than 500 characters",
		"definition_of_done.module_shape line 1 must be no more than 88 characters",
		"verification.automated[0].text must be no more than 88 characters",
	}
	for _, substr := range want {
		found := false
		for _, err := range errs {
			if strings.Contains(err.Message, substr) {
				found = true
				break
			}
		}
		if !found {
			t.Fatalf("aggregate errors missing %q: %+v", substr, errs)
		}
	}
}

func TestValidatePlanAllReturnsEmptyOnValidPlan(t *testing.T) {
	if errs := ValidatePlanAll(validPlan()); len(errs) != 0 {
		t.Fatalf("expected no violations, got %+v", errs)
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
