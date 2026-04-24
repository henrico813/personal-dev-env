package schema

import (
	"bytes"
	"encoding/json"
	"fmt"
)

type SchemaDocument struct {
	PlanExample     Plan     `json:"plan_example"`
	ValidationRules []string `json:"validation_rules"`
}

type Plan struct {
	Title            string           `json:"title"`
	Overview         string           `json:"overview"`
	DefinitionOfDone DefinitionOfDone `json:"definition_of_done"`
	Implementation   []Step           `json:"implementation"`
	Verification     *Verification    `json:"verification"`
}

// ChecklistStatus is a closed enum of completion states for a ChecklistItem.
// The empty string and StatusPending render as "- [ ]"; StatusDone renders as
// "- [x]". Any other value is rejected at decode and by ValidatePlan.
type ChecklistStatus string

const (
	StatusPending ChecklistStatus = "pending"
	StatusDone    ChecklistStatus = "done"
)

// ChecklistItem is one entry in a rendered checklist (goal or verification
// step). A plain-string JSON value decodes as ChecklistItem{Text: s} so
// existing callers keep working; object form must use {text, status}.
type ChecklistItem struct {
	Text   string          `json:"text"`
	Status ChecklistStatus `json:"status,omitempty"`
}

// IsDone reports whether the item is in the done state.
func (c ChecklistItem) IsDone() bool { return c.Status == StatusDone }

type DefinitionOfDone struct {
	Narrative    string          `json:"narrative"`
	Goals        []ChecklistItem `json:"goals"`
	CurrentState string          `json:"current_state"`
	ModuleShape  string          `json:"module_shape"`
}

type Step struct {
	Title       string       `json:"title"`
	Summary     string       `json:"summary"`
	FileChanges []FileChange `json:"file_changes"`
}

type FileChange struct {
	Filename    string `json:"filename"`
	Explanation string `json:"explanation"`
	Diff        string `json:"diff"`
}

type Verification struct {
	Summary   string          `json:"summary"`
	Automated []ChecklistItem `json:"automated"`
	Manual    []ChecklistItem `json:"manual"`
}

func DecodePlan(data []byte) (Plan, error) {
	dec := json.NewDecoder(bytes.NewReader(data))
	dec.DisallowUnknownFields()
	var plan Plan
	if err := dec.Decode(&plan); err != nil {
		return Plan{}, err
	}
	if dec.More() {
		return Plan{}, fmt.Errorf("trailing data after plan JSON")
	}
	return plan, nil
}

// UnmarshalJSON accepts either a plain string (legacy payloads) or a strict
// {"text":...,"status":...} object. Plain strings decode with empty Status,
// rendered unchecked. Object form uses a strict decoder so typos in field
// names (e.g. "stats" instead of "status") fail loudly rather than silently
// round-tripping as unchecked, matching decodeStrictJSON in replace.go.
func (c *ChecklistItem) UnmarshalJSON(data []byte) error {
	if len(data) > 0 && data[0] == '"' {
		var s string
		if err := json.Unmarshal(data, &s); err != nil {
			return err
		}
		c.Text = s
		c.Status = ""
		return nil
	}
	type raw ChecklistItem
	var r raw
	dec := json.NewDecoder(bytes.NewReader(data))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&r); err != nil {
		return err
	}
	switch r.Status {
	case "", StatusPending:
		*c = ChecklistItem(r)
		c.Status = ""
		return nil
	case StatusDone:
		*c = ChecklistItem(r)
		return nil
	default:
		return fmt.Errorf("invalid checklist item status %q: want pending or done", r.Status)
	}
}

func BuildPlanExample() Plan {
	return Plan{
		Title:    "Short title for the plan",
		Overview: "2-4 sentence summary of what the plan changes and why.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Paragraph describing why the change matters and how the pieces fit together.",
			Goals:        []ChecklistItem{{Text: "Concrete acceptance criterion"}},
			CurrentState: "Current behavior, constraints, and relevant file:line references.",
			ModuleShape:  "Target file and directory structure after the change.",
		},
		Implementation: []Step{
			{
				Title:   "Short step title",
				Summary: "Narrative summary explaining what this step changes and why.",
				FileChanges: []FileChange{
					{
						Filename:    "path/to/file.ext",
						Explanation: "One sentence explaining why this code exists.",
						Diff:        "Unified diff of the change to this file, with context lines.",
					},
				},
			},
		},
		Verification: &Verification{
			Summary:   "Optional summary describing how verification maps to the goals.",
			Automated: []ChecklistItem{{Text: "Runnable automated check"}},
			Manual:    []ChecklistItem{{Text: "Manual verification step"}},
		},
	}
}

func ValidationRules() []string {
	return []string{
		"title, overview, definition_of_done.narrative, definition_of_done.current_state, and definition_of_done.module_shape must be non-empty",
		"definition_of_done.goals must contain between 1 and 8 items",
		"each definition_of_done.goals item must be at most 88 characters",
		"implementation must contain at least one step",
		"each implementation step must include a title, summary, and at least one file change",
		"each file change must include a filename, explanation, and diff",
		"verification must be present",
	}
}

func BuildSchemaJSON() string {
	doc := SchemaDocument{
		PlanExample:     BuildPlanExample(),
		ValidationRules: ValidationRules(),
	}

	raw, _ := json.MarshalIndent(doc, "", "  ")
	return string(raw)
}
