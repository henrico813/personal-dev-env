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
	plan.DefinitionOfDone.Goals = make([]schema.ChecklistItem, schema.MaxDoDGoals)
	for i := range plan.DefinitionOfDone.Goals {
		plan.DefinitionOfDone.Goals[i] = schema.ChecklistItem{Text: strings.Repeat("a", schema.MaxDoDGoalLength)}
	}

	if err := ValidatePlan(plan); err != nil {
		t.Fatalf("ValidatePlan() error = %v, want nil", err)
	}
}

func TestValidatePlanRejectsTooManyGoals(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = make([]schema.ChecklistItem, schema.MaxDoDGoals+1)
	for i := range plan.DefinitionOfDone.Goals {
		plan.DefinitionOfDone.Goals[i] = schema.ChecklistItem{Text: "short goal"}
	}

	err := ValidatePlan(plan)
	if err == nil {
		t.Fatal("ValidatePlan() error = nil, want error")
	}
<<<<<<< HEAD
	if got, want := err.Error(), fmt.Sprintf("definition_of_done.goals must have no more than %d goals (got %d)", schema.MaxDoDGoals, schema.MaxDoDGoals+1); got != want {
		t.Fatalf("ValidatePlan() error = %q, want %q", got, want)
	}
}

func TestValidatePlanRejectsGoalLongerThanLimit(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = []schema.ChecklistItem{{Text: strings.Repeat("a", schema.MaxDoDGoalLength+1)}}

	err := ValidatePlan(plan)
	if err == nil {
		t.Fatal("ValidatePlan() error = nil, want error")
	}
<<<<<<< HEAD
	if got, want := err.Error(), fmt.Sprintf("definition_of_done.goals[0] must be no more than %d characters (got %d)", schema.MaxDoDGoalLength, schema.MaxDoDGoalLength+1); got != want {
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
		plan.DefinitionOfDone.Goals = make([]schema.ChecklistItem, schema.MaxDoDGoals+2)
		for i := range plan.DefinitionOfDone.Goals {
			plan.DefinitionOfDone.Goals[i] = schema.ChecklistItem{Text: fmt.Sprintf("goal-%d", i)}
		}
		err := ValidatePlan(plan)
		if err == nil {
			t.Fatal("expected error")
		}
<<<<<<< HEAD
		if !strings.Contains(err.Error(), fmt.Sprintf("(got %d)", schema.MaxDoDGoals+2)) {
			t.Fatalf("error %q does not contain actual count", err.Error())
		}
	})

	t.Run("goal_length", func(t *testing.T) {
		plan := validPlan()
		plan.DefinitionOfDone.Goals = []schema.ChecklistItem{{Text: strings.Repeat("a", schema.MaxDoDGoalLength+1)}}
		err := ValidatePlan(plan)
		if err == nil {
			t.Fatal("expected error")
		}
<<<<<<< HEAD
		if !strings.Contains(err.Error(), fmt.Sprintf("(got %d)", schema.MaxDoDGoalLength+1)) {
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
				p.Title = strings.Repeat("t", schema.MaxTitleLength+1)
			},
<<<<<<< HEAD
			wantSubstr: fmt.Sprintf("title must be no more than %d characters", schema.MaxTitleLength),
		},
		{
			name: "overview",
			mutate: func(p *schema.Plan) {
				p.Overview = strings.Repeat("o", schema.MaxOverviewLength+1)
			},
<<<<<<< HEAD
			wantSubstr: fmt.Sprintf("overview must be no more than %d characters", schema.MaxOverviewLength),
		},
		{
			name: "narrative",
			mutate: func(p *schema.Plan) {
				p.DefinitionOfDone.Narrative = strings.Repeat("n", schema.MaxDoDNarrativeLength+1)
			},
<<<<<<< HEAD
			wantSubstr: fmt.Sprintf("definition_of_done.narrative must be no more than %d characters", schema.MaxDoDNarrativeLength),
		},
		{
			name: "current_state",
			mutate: func(p *schema.Plan) {
				p.DefinitionOfDone.CurrentState = strings.Repeat("c", schema.MaxCurrentStateLength+1)
			},
<<<<<<< HEAD
			wantSubstr: fmt.Sprintf("definition_of_done.current_state must be no more than %d characters", schema.MaxCurrentStateLength),
		},
		{
			name: "module_shape_line",
			mutate: func(p *schema.Plan) {
				p.DefinitionOfDone.ModuleShape = strings.Repeat("m", schema.MaxModuleShapeLineLength+1)
			},
<<<<<<< HEAD
			wantSubstr: fmt.Sprintf("definition_of_done.module_shape line 1 must be no more than %d characters", schema.MaxModuleShapeLineLength),
		},
		{
			name: "step_title",
			mutate: func(p *schema.Plan) {
				p.Implementation[0].Title = strings.Repeat("s", schema.MaxStepTitleLength+1)
			},
<<<<<<< HEAD
			wantSubstr: fmt.Sprintf("implementation[0].title must be no more than %d characters", schema.MaxStepTitleLength),
		},
		{
			name: "step_summary",
			mutate: func(p *schema.Plan) {
				p.Implementation[0].Summary = strings.Repeat("s", schema.MaxStepSummaryLength+1)
			},
<<<<<<< HEAD
			wantSubstr: fmt.Sprintf("implementation[0].summary must be no more than %d characters", schema.MaxStepSummaryLength),
		},
		{
			name: "file_change_explanation",
			mutate: func(p *schema.Plan) {
				p.Implementation[0].FileChanges[0].Explanation = strings.Repeat("e", schema.MaxFileChangeExplanationLength+1)
			},
<<<<<<< HEAD
			wantSubstr: fmt.Sprintf("file_changes[0].explanation must be no more than %d characters", schema.MaxFileChangeExplanationLength),
		},
		{
			name: "verification_automated_text",
			mutate: func(p *schema.Plan) {
				p.Verification.Automated = []schema.ChecklistItem{{Text: strings.Repeat("a", schema.MaxVerificationItemTextLength+1)}}
			},
<<<<<<< HEAD
			wantSubstr: fmt.Sprintf("verification.automated[0].text must be no more than %d characters", schema.MaxVerificationItemTextLength),
		},
		{
			name: "verification_manual_text",
			mutate: func(p *schema.Plan) {
				p.Verification.Manual = []schema.ChecklistItem{{Text: strings.Repeat("m", schema.MaxVerificationItemTextLength+1)}}
			},
<<<<<<< HEAD
			wantSubstr: fmt.Sprintf("verification.manual[0].text must be no more than %d characters", schema.MaxVerificationItemTextLength),
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
	plan.Overview = strings.Repeat("o", schema.MaxOverviewLength+1)
	plan.DefinitionOfDone.Narrative = strings.Repeat("n", schema.MaxDoDNarrativeLength+1)
	plan.DefinitionOfDone.CurrentState = strings.Repeat("c", schema.MaxCurrentStateLength+1)
	plan.DefinitionOfDone.ModuleShape = strings.Repeat("m", schema.MaxModuleShapeLineLength+1)
	plan.Verification.Automated = []schema.ChecklistItem{{Text: strings.Repeat("a", schema.MaxVerificationItemTextLength+1)}}

	errs := ValidatePlanAll(plan)
	if len(errs) < 5 {
		t.Fatalf("expected at least 5 violations, got %d: %+v", len(errs), errs)
	}

	want := []string{
		"title is required",
<<<<<<< HEAD
		fmt.Sprintf("overview must be no more than %d characters", schema.MaxOverviewLength),
		fmt.Sprintf("definition_of_done.narrative must be no more than %d characters", schema.MaxDoDNarrativeLength),
		fmt.Sprintf("definition_of_done.current_state must be no more than %d characters", schema.MaxCurrentStateLength),
		fmt.Sprintf("definition_of_done.module_shape line 1 must be no more than %d characters", schema.MaxModuleShapeLineLength),
		fmt.Sprintf("verification.automated[0].text must be no more than %d characters", schema.MaxVerificationItemTextLength),
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
