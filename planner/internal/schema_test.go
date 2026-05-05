package internal

import (
	"encoding/json"
	"fmt"
	"strings"
	"testing"
)

func planJSON(overrides map[string]string, trailing string) []byte {
	values := map[string]string{
		"title":                `"t"`,
		"overview":             `"o"`,
		"narrative":            `"n"`,
		"goals":                `["g"]`,
		"current_state":        `"s"`,
		"module_shape":         `"m"`,
		"step_title":           `"T"`,
		"step_summary":         `"S"`,
		"filename":             `"f"`,
		"explanation":          `"e"`,
		"diff":                 `"d"`,
		"verification_summary": `""`,
		"automated":            `["x"]`,
		"manual":               `["y"]`,
		"extra_field":          ``,
	}
	for key, value := range overrides {
		values[key] = value
	}
	return []byte(fmt.Sprintf(`{
		"title": %s,
		"overview": %s,
		"definition_of_done": {
			"narrative": %s,
			"goals": %s,
			"current_state": %s,
			"module_shape": %s
		},
		"implementation": [{
			"title": %s,
			"summary": %s,
			"file_changes": [{
				"filename": %s,
				"explanation": %s,
				"diff": %s
			}]
		}],
		"verification": {
			"summary": %s,
			"automated": %s,
			"manual": %s
		}%s
	}%s`,
		values["title"],
		values["overview"],
		values["narrative"],
		values["goals"],
		values["current_state"],
		values["module_shape"],
		values["step_title"],
		values["step_summary"],
		values["filename"],
		values["explanation"],
		values["diff"],
		values["verification_summary"],
		values["automated"],
		values["manual"],
		values["extra_field"],
		trailing,
	))
}

func TestChecklistItemDecodes(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  ChecklistItem
	}{
		{"plain_string", `"goal"`, ChecklistItem{Text: "goal"}},
		{"object_empty_status", `{"text":"goal"}`, ChecklistItem{Text: "goal"}},
		{"object_pending_normalizes_to_empty", `{"text":"g","status":"pending"}`, ChecklistItem{Text: "g"}},
		{"object_done", `{"text":"g","status":"done"}`, ChecklistItem{Text: "g", Status: StatusDone}},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			var got ChecklistItem
			if err := json.Unmarshal([]byte(tc.input), &got); err != nil {
				t.Fatalf("unmarshal: %v", err)
			}
			if got != tc.want {
				t.Fatalf("got %+v want %+v", got, tc.want)
			}
		})
	}
}

func TestChecklistItemRejectsInput(t *testing.T) {
	tests := []struct {
		name    string
		input   string
		wantErr string
	}{
		{name: "invalid_status", input: `{"text":"x","status":"started"}`, wantErr: "invalid checklist item status"},
		{name: "unknown_field", input: `{"text":"x","stats":"done"}`, wantErr: "unknown field"},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			var got ChecklistItem
			err := json.Unmarshal([]byte(tc.input), &got)
			if err == nil {
				t.Fatal("expected error")
			}
			if !strings.Contains(err.Error(), tc.wantErr) {
				t.Fatalf("error %q does not contain %q", err.Error(), tc.wantErr)
			}
		})
	}
}

func TestDecodePlanAcceptsGoals(t *testing.T) {
	input := planJSON(map[string]string{
		"goals":                `["a","b"]`,
		"verification_summary": `"v"`,
	}, "")
	plan, err := DecodePlan(input)
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	if len(plan.DefinitionOfDone.Goals) != 2 || plan.DefinitionOfDone.Goals[0].Text != "a" {
		t.Fatalf("goals: %+v", plan.DefinitionOfDone.Goals)
	}
	if plan.DefinitionOfDone.Goals[1].Status != "" {
		t.Fatalf("plain-string goal must have empty status: %+v", plan.DefinitionOfDone.Goals[1])
	}
	if plan.Verification.Automated[0].Text != "x" || plan.Verification.Manual[0].Text != "y" {
		t.Fatalf("verification: %+v", plan.Verification)
	}
}

func TestDecodePlanRejectsInput(t *testing.T) {
	tests := []struct {
		name      string
		overrides map[string]string
		trailing  string
		wantErr   string
	}{
		{
			name: "top_level_unknown_field",
			overrides: map[string]string{
				"extra_field": `, "extra_field": "boom"`,
			},
			wantErr: "unknown field",
		},
		{
			name:      "trailing_data",
			overrides: nil,
			trailing:  " trailing",
			wantErr:   "trailing data after plan JSON",
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			_, err := DecodePlan(planJSON(tc.overrides, tc.trailing))
			if err == nil {
				t.Fatal("expected error")
			}
			if !strings.Contains(err.Error(), tc.wantErr) {
				t.Fatalf("error %q does not contain %q", err.Error(), tc.wantErr)
			}
		})
	}
}

func TestDecodePlanNormalizesStatus(t *testing.T) {
	input := planJSON(map[string]string{
		"goals": `[{"text":"a","status":"pending"},"b"]`,
	}, "")
	plan, err := DecodePlan(input)
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	for i, g := range plan.DefinitionOfDone.Goals {
		if g.Status != "" {
			t.Fatalf("goal[%d] must have empty status after normalization, got %q", i, g.Status)
		}
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
		{"dod_module_shape", MaxModuleShapeLineLength, fmt.Sprintf("each line of definition_of_done.module_shape must be at most %d characters", MaxModuleShapeLineLength)},
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
