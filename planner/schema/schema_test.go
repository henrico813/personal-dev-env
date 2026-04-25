package schema

import (
	"encoding/json"
	"strings"
	"testing"
)

func basePlan() Plan {
	return Plan{
		Title:    "t",
		Overview: "o",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "n",
			Goals:        []ChecklistItem{{Text: "g"}},
			CurrentState: "s",
			ModuleShape:  "m",
		},
		Implementation: []Step{{
			Title:   "T",
			Summary: "S",
			FileChanges: []FileChange{{
				Filename:    "f",
				Explanation: "e",
				Diff:        "d",
			}},
		}},
		Verification: &Verification{
			Summary:   "",
			Automated: []ChecklistItem{{Text: "x"}},
			Manual:    []ChecklistItem{{Text: "y"}},
		},
	}
}

func mustJSON(t *testing.T, v any) []byte {
	t.Helper()
	raw, err := json.Marshal(v)
	if err != nil {
		t.Fatalf("json.Marshal: %v", err)
	}
	return raw
}

func mustJSONMap(t *testing.T, plan Plan, kv map[string]any) []byte {
	t.Helper()
	var doc map[string]any
	raw := mustJSON(t, plan)
	if err := json.Unmarshal(raw, &doc); err != nil {
		t.Fatalf("json.Unmarshal: %v", err)
	}
	for key, value := range kv {
		doc[key] = value
	}
	return mustJSON(t, doc)
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
	input := basePlan()
	input.DefinitionOfDone.Goals = []ChecklistItem{{Text: "a"}, {Text: "b"}}
	input.Verification.Summary = "v"
	plan, err := DecodePlan(mustJSON(t, input))
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
		name    string
		input   func(t *testing.T) []byte
		wantErr string
	}{
		{
			name: "top_level_unknown_field",
			input: func(t *testing.T) []byte {
				return mustJSONMap(t, basePlan(), map[string]any{"extra_field": "boom"})
			},
			wantErr: "unknown field",
		},
		{
			name:    "trailing_data",
			input:   func(t *testing.T) []byte { return append(mustJSON(t, basePlan()), []byte(" trailing")...) },
			wantErr: "trailing data after plan JSON",
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			_, err := DecodePlan(tc.input(t))
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
	input := map[string]any{
		"title":    "T",
		"overview": "O",
		"definition_of_done": map[string]any{
			"narrative":     "N",
			"goals":         []any{map[string]any{"text": "a", "status": "pending"}, "b"},
			"current_state": "C",
			"module_shape":  "M",
		},
		"implementation": []any{
			map[string]any{
				"title":   "T",
				"summary": "S",
				"file_changes": []any{
					map[string]any{
						"filename":    "f",
						"explanation": "e",
						"diff":        "@@ -1 +1 @@\n-x\n+y",
					},
				},
			},
		},
		"verification": map[string]any{
			"summary":   "",
			"automated": []any{"x"},
			"manual":    []any{"y"},
		},
	}
	plan, err := DecodePlan(mustJSON(t, input))
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	for i, g := range plan.DefinitionOfDone.Goals {
		if g.Status != "" {
			t.Fatalf("goal[%d] must have empty status after normalization, got %q", i, g.Status)
		}
	}
}
