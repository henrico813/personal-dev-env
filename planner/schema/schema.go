package schema

import (
	"encoding/json"
)

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
	Language    string `json:"language"`
	Code        string `json:"code"`
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

func BuildSchemaJSON() string {
	schema := map[string]any{
		"type":        "object",
		"title":       "planner create input contract",
		"description": "Built-in documentation for the planner contract, based on PDEV-008 and requiring a top-level verification field.",
		"required":    []string{"title", "overview", "definition_of_done", "implementation", "verification"},
		"properties": map[string]any{
			"title":              requiredString(),
			"overview":           requiredString(),
			"definition_of_done": definitionOfDoneSchema(),
			"implementation":     implementationSchema(),
			"verification":       verificationSchema(),
		},
		"contract": map[string]any{
			"commands": map[string]any{
				"help":        "Print built-in usage and workflow guidance.",
				"show-schema": "Print this full contract, including nested JSON shape, current validator rules, and command semantics.",
				"validate":    "Validate planner JSON input without rendering markdown. Usage: planner validate <plan.json>.",
				"create":      "Render canonical markdown from valid planner JSON. Usage: planner create <plan.json> <output.md>.",
			},
			"guarantees": []string{
				"validate and create use the same structural validation rules",
				"planner preserves the current PDEV-008 JSON decode behavior",
				"create rejects invalid JSON input before rendering",
				"create renders markdown-only output and does not embed planner source JSON appendices",
				"current validator requires non-empty title, overview, definition_of_done.narrative, current_state, and module_shape",
				"current validator requires at least one goal and at least one implementation step",
				"current validator requires each implementation step to include title, summary, and at least one file change",
				"current validator requires each file change to include filename, explanation, language, and code",
				"current validator requires the top-level verification field to be present",
				"verification.summary, verification.automated, and verification.manual remain optional",
			},
		},
	}

	raw, _ := json.MarshalIndent(schema, "", "  ")
	return string(raw)
}

func requiredString() map[string]any {
	return map[string]any{
		"type":      "string",
		"minLength": 1,
	}
}

func stringArray() map[string]any {
	return map[string]any{
		"type":  "array",
		"items": requiredString(),
	}
}

func nonEmptyStringArray() map[string]any {
	return map[string]any{
		"type":     "array",
		"minItems": 1,
		"items":    requiredString(),
	}
}

func definitionOfDoneSchema() map[string]any {
	return map[string]any{
		"type":     "object",
		"required": []string{"narrative", "goals", "current_state", "module_shape"},
		"properties": map[string]any{
			"narrative":     requiredString(),
			"goals":         nonEmptyStringArray(),
			"current_state": requiredString(),
			"module_shape":  requiredString(),
		},
	}
}

func implementationSchema() map[string]any {
	return map[string]any{
		"type":     "array",
		"minItems": 1,
		"items": map[string]any{
			"type":     "object",
			"required": []string{"title", "summary", "file_changes"},
			"properties": map[string]any{
				"title":        requiredString(),
				"summary":      requiredString(),
				"file_changes": fileChangesSchema(),
			},
		},
	}
}

func fileChangesSchema() map[string]any {
	return map[string]any{
		"type":     "array",
		"minItems": 1,
		"items": map[string]any{
			"type":     "object",
			"required": []string{"filename", "explanation", "language", "code"},
			"properties": map[string]any{
				"filename":    requiredString(),
				"explanation": requiredString(),
				"language":    requiredString(),
				"code":        requiredString(),
			},
		},
	}
}

func verificationSchema() map[string]any {
	return map[string]any{
		"type": "object",
		"properties": map[string]any{
			"summary":   map[string]any{"type": "string"},
			"automated": stringArray(),
			"manual":    stringArray(),
		},
	}
}
