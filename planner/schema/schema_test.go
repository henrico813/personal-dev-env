package schema

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
