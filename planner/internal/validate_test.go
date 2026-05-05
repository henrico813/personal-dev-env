package internal

import (
	"fmt"
	"strings"
	"testing"

)

func validPlan() Plan {
	return Plan{
		Title:    "Planner validation rules",
		Overview: "Add goal-count and goal-length validation for rendered plans.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Goals should stay short and capped so the rendered markdown stays scannable.",
			Goals:        []ChecklistItem{{Text: "Rendered plan goals follow the configured limits."}},
			CurrentState: "The planner currently requires at least one goal but does not cap count or length.",
			ModuleShape:  "Validation remains centralized in planner/internal.",
		},
		Implementation: []Step{
			{
				Title:   "Validate goals",
				Summary: "Reject plans with too many goals or goals that are too long.",
				FileChanges: []FileChange{
					{
						Filename:    "planner/internal/validate.go",
						Explanation: "Adds the markdown goal constraints.",
						Diff:        "@@ -1 +1 @@\n- old\n+ new",
					},
				},
			},
		},
		Verification: &Verification{},
	}
}

func TestValidatePlanAcceptsGoalLimits(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = make([]ChecklistItem, MaxDoDGoals)
	for i := range plan.DefinitionOfDone.Goals {
		plan.DefinitionOfDone.Goals[i] = ChecklistItem{Text: strings.Repeat("a", MaxDoDGoalLength)}
	}

	if err := ValidatePlan(plan); err != nil {
		t.Fatalf("ValidatePlan() error = %v, want nil", err)
	}
}

func TestValidatePlanRejectsTooManyGoals(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = make([]ChecklistItem, MaxDoDGoals+1)
	for i := range plan.DefinitionOfDone.Goals {
		plan.DefinitionOfDone.Goals[i] = ChecklistItem{Text: "short goal"}
	}

	err := ValidatePlan(plan)
	if err == nil {
		t.Fatal("ValidatePlan() error = nil, want error")
	}
	if got, want := err.Error(), fmt.Sprintf("definition_of_done.goals must have no more than %d goals (got %d)", MaxDoDGoals, MaxDoDGoals+1); got != want {
		t.Fatalf("ValidatePlan() error = %q, want %q", got, want)
	}
}

func TestValidatePlanRejectsGoalLongerThanLimit(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = []ChecklistItem{{Text: strings.Repeat("a", MaxDoDGoalLength+1)}}

	err := ValidatePlan(plan)
	if err == nil {
		t.Fatal("ValidatePlan() error = nil, want error")
	}
	if got, want := err.Error(), fmt.Sprintf("definition_of_done.goals[0] must be no more than %d characters (got %d)", MaxDoDGoalLength, MaxDoDGoalLength+1); got != want {
		t.Fatalf("ValidatePlan() error = %q, want %q", got, want)
	}
}

func TestValidatePlanRejectsInvalidGoalStatus(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = []ChecklistItem{{Text: "goal", Status: "started"}}
	if err := ValidatePlan(plan); err == nil {
		t.Fatal("expected error for invalid goal status")
	}
}

func TestValidatePlanRejectsEmptyGoalText(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = []ChecklistItem{{Text: "  "}}
	if err := ValidatePlan(plan); err == nil {
		t.Fatal("expected error for empty goal text")
	}
}

func TestValidatePlanRejectsEmptyVerificationItemText(t *testing.T) {
	plan := validPlan()
	plan.Verification = &Verification{
		Automated: []ChecklistItem{{Text: ""}},
		Manual:    []ChecklistItem{{Text: "m"}},
	}
	if err := ValidatePlan(plan); err == nil {
		t.Fatal("expected error for empty automated text")
	}
}

func TestValidatePlanRejectsInvalidVerificationItemStatus(t *testing.T) {
	plan := validPlan()
	plan.Verification = &Verification{
		Automated: []ChecklistItem{{Text: "a", Status: "invalid"}},
		Manual:    []ChecklistItem{{Text: "m"}},
	}
	if err := ValidatePlan(plan); err == nil {
		t.Fatal("expected error for invalid automated status")
	}
}

func TestValidatePlanLengthMessagesIncludeActual(t *testing.T) {
	t.Run("goals_count", func(t *testing.T) {
		plan := validPlan()
		plan.DefinitionOfDone.Goals = make([]ChecklistItem, MaxDoDGoals+2)
		for i := range plan.DefinitionOfDone.Goals {
			plan.DefinitionOfDone.Goals[i] = ChecklistItem{Text: fmt.Sprintf("goal-%d", i)}
		}
		err := ValidatePlan(plan)
		if err == nil {
			t.Fatal("expected error")
		}
		if !strings.Contains(err.Error(), fmt.Sprintf("(got %d)", MaxDoDGoals+2)) {
			t.Fatalf("error %q does not contain actual count", err.Error())
		}
	})

	t.Run("goal_length", func(t *testing.T) {
		plan := validPlan()
		plan.DefinitionOfDone.Goals = []ChecklistItem{{Text: strings.Repeat("a", MaxDoDGoalLength+1)}}
		err := ValidatePlan(plan)
		if err == nil {
			t.Fatal("expected error")
		}
		if !strings.Contains(err.Error(), fmt.Sprintf("(got %d)", MaxDoDGoalLength+1)) {
			t.Fatalf("error %q does not contain actual length", err.Error())
		}
	})
}

func TestValidatePlanRejectsNewLengthCaps(t *testing.T) {
	tests := []struct {
		name       string
		mutate     func(*Plan)
		wantSubstr string
	}{
		{
			name: "title",
			mutate: func(p *Plan) {
				p.Title = strings.Repeat("t", MaxTitleLength+1)
			},
			wantSubstr: fmt.Sprintf("title must be no more than %d characters", MaxTitleLength),
		},
		{
			name: "overview",
			mutate: func(p *Plan) {
				p.Overview = strings.Repeat("o", MaxOverviewLength+1)
			},
			wantSubstr: fmt.Sprintf("overview must be no more than %d characters", MaxOverviewLength),
		},
		{
			name: "narrative",
			mutate: func(p *Plan) {
				p.DefinitionOfDone.Narrative = strings.Repeat("n", MaxDoDNarrativeLength+1)
			},
			wantSubstr: fmt.Sprintf("definition_of_done.narrative must be no more than %d characters", MaxDoDNarrativeLength),
		},
		{
			name: "current_state",
			mutate: func(p *Plan) {
				p.DefinitionOfDone.CurrentState = strings.Repeat("c", MaxCurrentStateLength+1)
			},
			wantSubstr: fmt.Sprintf("definition_of_done.current_state must be no more than %d characters", MaxCurrentStateLength),
		},
		{
			name: "module_shape_line",
			mutate: func(p *Plan) {
				p.DefinitionOfDone.ModuleShape = strings.Repeat("m", MaxModuleShapeLineLength+1)
			},
			wantSubstr: fmt.Sprintf("definition_of_done.module_shape line 1 must be no more than %d characters", MaxModuleShapeLineLength),
		},
		{
			name: "step_title",
			mutate: func(p *Plan) {
				p.Implementation[0].Title = strings.Repeat("s", MaxStepTitleLength+1)
			},
			wantSubstr: fmt.Sprintf("implementation[0].title must be no more than %d characters", MaxStepTitleLength),
		},
		{
			name: "step_summary",
			mutate: func(p *Plan) {
				p.Implementation[0].Summary = strings.Repeat("s", MaxStepSummaryLength+1)
			},
			wantSubstr: fmt.Sprintf("implementation[0].summary must be no more than %d characters", MaxStepSummaryLength),
		},
		{
			name: "file_change_explanation",
			mutate: func(p *Plan) {
				p.Implementation[0].FileChanges[0].Explanation = strings.Repeat("e", MaxFileChangeExplanationLength+1)
			},
			wantSubstr: fmt.Sprintf("file_changes[0].explanation must be no more than %d characters", MaxFileChangeExplanationLength),
		},
		{
			name: "verification_automated_text",
			mutate: func(p *Plan) {
				p.Verification.Automated = []ChecklistItem{{Text: strings.Repeat("a", MaxVerificationItemTextLength+1)}}
			},
			wantSubstr: fmt.Sprintf("verification.automated[0].text must be no more than %d characters", MaxVerificationItemTextLength),
		},
		{
			name: "verification_manual_text",
			mutate: func(p *Plan) {
				p.Verification.Manual = []ChecklistItem{{Text: strings.Repeat("m", MaxVerificationItemTextLength+1)}}
			},
			wantSubstr: fmt.Sprintf("verification.manual[0].text must be no more than %d characters", MaxVerificationItemTextLength),
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
	plan.Overview = strings.Repeat("o", MaxOverviewLength+1)
	plan.DefinitionOfDone.Narrative = strings.Repeat("n", MaxDoDNarrativeLength+1)
	plan.DefinitionOfDone.CurrentState = strings.Repeat("c", MaxCurrentStateLength+1)
	plan.DefinitionOfDone.ModuleShape = strings.Repeat("m", MaxModuleShapeLineLength+1)
	plan.Verification.Automated = []ChecklistItem{{Text: strings.Repeat("a", MaxVerificationItemTextLength+1)}}

	errs := ValidatePlanAll(plan)
	if len(errs) < 5 {
		t.Fatalf("expected at least 5 violations, got %d: %+v", len(errs), errs)
	}

	want := []string{
		"title is required",
		fmt.Sprintf("overview must be no more than %d characters", MaxOverviewLength),
		fmt.Sprintf("definition_of_done.narrative must be no more than %d characters", MaxDoDNarrativeLength),
		fmt.Sprintf("definition_of_done.current_state must be no more than %d characters", MaxCurrentStateLength),
		fmt.Sprintf("definition_of_done.module_shape line 1 must be no more than %d characters", MaxModuleShapeLineLength),
		fmt.Sprintf("verification.automated[0].text must be no more than %d characters", MaxVerificationItemTextLength),
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
			filename: "planner/internal/validate.go",
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
