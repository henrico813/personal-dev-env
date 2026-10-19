package internal

import (
	"fmt"
	"strings"
	"testing"
)

func TestValidatePlanRejectsDuplicateStepFilenames(t *testing.T) {
	plan := Plan{
		Title:    "T",
		Overview: "O",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "N",
			Goals:        []ChecklistItem{{Text: "g"}},
			CurrentState: "C",
			ModuleShape:  "M",
		},
		Implementation: []Step{{
			Title:   "S",
			Summary: "sum",
			FileChanges: []FileChange{{
				Filename:    "f.go",
				Explanation: "e",
				Diff:        "@@ -1 +1 @@\n-a\n+b",
			}, {
				Filename:    "f.go",
				Explanation: "e2",
				Diff:        "@@ -1 +1 @@\n-a\n+b",
			}},
		}},
		Verification: &Verification{Summary: "v"},
	}
	if err := ValidatePlan(plan); err == nil || !strings.Contains(err.Error(), "duplicate filename") {
		t.Fatalf("ValidatePlan() err = %v", err)
	}
}

func TestValidationRulesUseExportedLimits(t *testing.T) {
	rules := ValidationRules()

	tests := []struct {
		desc     string
		limit    int
		wantRule string
	}{
		{"title", MaxTitleLength, fmt.Sprintf("title must be at most %d characters", MaxTitleLength)},
		{"overview", MaxOverviewLength, fmt.Sprintf("overview must be at most %d characters", MaxOverviewLength)},
		{"dod_narrative", MaxDoDNarrativeLength, fmt.Sprintf("definition_of_done.narrative must be at most %d characters", MaxDoDNarrativeLength)},
		{"dod_current_state", MaxCurrentStateLength, fmt.Sprintf("definition_of_done.current_state must be at most %d characters", MaxCurrentStateLength)},
		{"dod_module_shape", MaxModuleShapeLineLength, fmt.Sprintf("each line of definition_of_done.module_shape before its first ### heading must be at most %d characters", MaxModuleShapeLineLength)},
		{"dod_goals_count", MaxDoDGoals, fmt.Sprintf("definition_of_done.goals must contain between 1 and %d items", MaxDoDGoals)},
		{"dod_goal_length", MaxDoDGoalLength, fmt.Sprintf("each definition_of_done.goals item must be at most %d characters", MaxDoDGoalLength)},
		{"step_title", MaxStepTitleLength, fmt.Sprintf("each implementation step title must be at most %d characters", MaxStepTitleLength)},
		{"step_summary", MaxStepSummaryLength, fmt.Sprintf("each implementation step summary must be at most %d characters", MaxStepSummaryLength)},
		{"file_explanation", MaxFileChangeExplanationLength, fmt.Sprintf("each file change explanation must be at most %d characters", MaxFileChangeExplanationLength)},
		{"verification_item", MaxVerificationItemTextLength, fmt.Sprintf("each verification.automated[i].text must be at most %d characters", MaxVerificationItemTextLength)},
	}

	ruleStrings := make(map[string]bool)
	for _, r := range rules {
		ruleStrings[r] = true
	}

	for _, tc := range tests {
		if !ruleStrings[tc.wantRule] {
			t.Fatalf("ValidationRules() missing %q (testing %s)", tc.wantRule, tc.desc)
		}
	}
}

func TestBuildPlanTemplateUsesExportedLimits(t *testing.T) {
	tmpl := BuildPlanTemplate()

	tests := []struct {
		desc       string
		gotString  string
		wantSubstr string
	}{
		{"title", tmpl.Title, fmt.Sprintf("max %d chars", MaxTitleLength)},
		{"overview", tmpl.Overview, fmt.Sprintf("max %d chars", MaxOverviewLength)},
		{"dod_narrative", tmpl.DefinitionOfDone.Narrative, fmt.Sprintf("max %d chars", MaxDoDNarrativeLength)},
		{"dod_current_state", tmpl.DefinitionOfDone.CurrentState, fmt.Sprintf("max %d chars", MaxCurrentStateLength)},
		{"dod_module_shape", tmpl.DefinitionOfDone.ModuleShape, fmt.Sprintf("each line <= %d chars", MaxModuleShapeLineLength)},
		{"dod_goals", tmpl.DefinitionOfDone.Goals[0].Text, fmt.Sprintf("1 to %d items", MaxDoDGoals)},
		{"dod_goal_length", tmpl.DefinitionOfDone.Goals[0].Text, fmt.Sprintf("<= %d chars", MaxDoDGoalLength)},
		{"step_title", tmpl.Implementation[0].Title, fmt.Sprintf("max %d chars", MaxStepTitleLength)},
		{"step_summary", tmpl.Implementation[0].Summary, fmt.Sprintf("max %d chars", MaxStepSummaryLength)},
		{"file_explanation", tmpl.Implementation[0].FileChanges[0].Explanation, fmt.Sprintf("max %d chars", MaxFileChangeExplanationLength)},
		{"verification_automated", tmpl.Verification.Automated[0].Text, fmt.Sprintf("max %d chars", MaxVerificationItemTextLength)},
		{"verification_manual", tmpl.Verification.Manual[0].Text, fmt.Sprintf("max %d chars", MaxVerificationItemTextLength)},
	}

	for _, tc := range tests {
		if !strings.Contains(tc.gotString, tc.wantSubstr) {
			t.Fatalf("BuildPlanTemplate() %s = %q does not contain %q", tc.desc, tc.gotString, tc.wantSubstr)
		}
	}
}
