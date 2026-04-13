package schema

import (
	"encoding/json"
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

type DefinitionOfDone struct {
	Narrative    string   `json:"narrative"`
	Goals        []string `json:"goals"`
	CurrentState string   `json:"current_state"`
	ModuleShape  string   `json:"module_shape"`
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
	Summary   string   `json:"summary"`
	Automated []string `json:"automated"`
	Manual    []string `json:"manual"`
}

func DecodePlan(data []byte) (Plan, error) {
	var plan Plan
	if err := json.Unmarshal(data, &plan); err != nil {
		return Plan{}, err
	}
	return plan, nil
}

func BuildPlanExample() Plan {
	return Plan{
		Title:    "Short title for the plan",
		Overview: "2-4 sentence summary of what the plan changes and why.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Paragraph describing why the change matters and how the pieces fit together.",
			Goals:        []string{"Concrete acceptance criterion"},
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
			Automated: []string{"Runnable automated check"},
			Manual:    []string{"Manual verification step"},
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
