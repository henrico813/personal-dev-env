package schema

import (
	"encoding/json"
	"testing"
)

func TestChecklistItemDecodesStringAndObject(t *testing.T) {
	tests := []struct {
		name  string
		input string
		want  ChecklistItem
	}{
		{"plain_string", `"goal"`, ChecklistItem{Text: "goal"}},
		{"object_empty_status", `{"text":"goal"}`, ChecklistItem{Text: "goal"}},
		{"object_pending", `{"text":"g","status":"pending"}`, ChecklistItem{Text: "g", Status: StatusPending}},
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

func TestChecklistItemRejectsInvalidStatus(t *testing.T) {
	var got ChecklistItem
	if err := json.Unmarshal([]byte(`{"text":"x","status":"started"}`), &got); err == nil {
		t.Fatal("expected error for invalid status")
	}
}

func TestChecklistItemRejectsUnknownField(t *testing.T) {
	var got ChecklistItem
	if err := json.Unmarshal([]byte(`{"text":"x","stats":"done"}`), &got); err == nil {
		t.Fatal("expected error for unknown field")
	}
}

func TestDecodePlanAcceptsPlainStringGoals(t *testing.T) {
	input := `{
		"title":"t","overview":"o",
		"definition_of_done":{"narrative":"n","goals":["a","b"],"current_state":"s","module_shape":"m"},
		"implementation":[{"title":"T","summary":"S","file_changes":[{"filename":"f","explanation":"e","diff":"d"}]}],
		"verification":{"summary":"v","automated":["x"],"manual":["y"]}
	}`
	plan, err := DecodePlan([]byte(input))
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
