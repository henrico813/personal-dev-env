package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
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

func validatePlan(plan Plan) error {
	if strings.TrimSpace(plan.Title) == "" {
		return errors.New("title is required")
	}
	if strings.TrimSpace(plan.Overview) == "" {
		return errors.New("overview is required")
	}
	if strings.TrimSpace(plan.DefinitionOfDone.Narrative) == "" {
		return errors.New("definition_of_done.narrative is required")
	}
	if len(plan.DefinitionOfDone.Goals) == 0 {
		return errors.New("at least one definition_of_done goal is required")
	}
	if strings.TrimSpace(plan.DefinitionOfDone.CurrentState) == "" {
		return errors.New("definition_of_done.current_state is required")
	}
	if strings.TrimSpace(plan.DefinitionOfDone.ModuleShape) == "" {
		return errors.New("definition_of_done.module_shape is required")
	}
	if len(plan.Implementation) == 0 {
		return errors.New("at least one implementation step is required")
	}
	if plan.Verification == nil {
		return errors.New("verification is required")
	}

	for _, step := range plan.Implementation {
		if strings.TrimSpace(step.Title) == "" {
			return errors.New("each implementation step needs a title")
		}
		if strings.TrimSpace(step.Summary) == "" {
			return errors.New("each implementation step needs a summary")
		}
		if len(step.FileChanges) == 0 {
			return errors.New("each implementation step needs file changes")
		}
		for _, change := range step.FileChanges {
			if strings.TrimSpace(change.Filename) == "" {
				return errors.New("each file change needs a filename")
			}
			if strings.TrimSpace(change.Explanation) == "" {
				return errors.New("each file change needs an explanation")
			}
			if strings.TrimSpace(change.Language) == "" {
				return errors.New("each file change needs a language")
			}
			if strings.TrimSpace(change.Code) == "" {
				return errors.New("each file change needs code")
			}
		}
	}

	return nil
}

func decodePlan(data []byte) (Plan, error) {
	var plan Plan
	if err := json.Unmarshal(data, &plan); err != nil {
		return Plan{}, err
	}
	return plan, nil
}

const planSourceBegin = "<!-- planner:source-begin -->"
const planSourceEnd = "<!-- planner:source-end -->"
const appendixHeader = "\n\n# Appendix\n\n## Plan JSON\n\n"

// appendPlanSource encodes the plan as indented JSON with SetEscapeHTML(true)
// so that '<' in any string value (including FileChange.Code) is escaped as
// \u003c. This makes it structurally impossible for the HTML-comment sentinels
// to appear inside any JSON string, preventing sentinel-collision bugs.
func appendPlanSource(rendered string, plan Plan) (string, error) {
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(true)
	enc.SetIndent("", "  ")
	if err := enc.Encode(plan); err != nil {
		return "", fmt.Errorf("marshal plan source: %w", err)
	}
	// enc.Encode appends a trailing newline; trim it so our sentinel framing is exact.
	raw := strings.TrimRight(buf.String(), "\n")
	return rendered + appendixHeader + planSourceBegin + "\n" + raw + "\n" + planSourceEnd + "\n", nil
}

// extractPlanSource scans from the END of mdContent via LastIndex to locate
// the sentinel pair and unmarshals the JSON between them. Using LastIndex
// plus HTML-escaped JSON guarantees no internal collision with the sentinels.
func extractPlanSource(mdContent string) (Plan, error) {
	beginIdx := strings.LastIndex(mdContent, planSourceBegin)
	if beginIdx == -1 {
		return Plan{}, errors.New("no Plan JSON appendix found in file")
	}
	start := beginIdx + len(planSourceBegin) + 1 // skip trailing newline
	endIdx := strings.LastIndex(mdContent, planSourceEnd)
	if endIdx == -1 || endIdx < start {
		return Plan{}, errors.New("Plan JSON appendix is not properly closed")
	}
	// trim trailing newline before end sentinel
	jsonBytes := []byte(strings.TrimRight(mdContent[start:endIdx], "\n"))
	return decodePlan(jsonBytes)
}

// createPlanFromStruct validates, renders, appends the JSON appendix, and
// atomically writes the output for the canonical full-plan workflow.
func createPlanFromStruct(plan Plan, outputPath string) error {
	if err := validatePlan(plan); err != nil {
		return fmt.Errorf("validate: %w", err)
	}
	rendered, err := renderPlan(plan)
	if err != nil {
		return fmt.Errorf("render: %w", err)
	}
	if err := verifyRenderedText(rendered, plan); err != nil {
		return fmt.Errorf("verify: %w", err)
	}
	withAppendix, err := appendPlanSource(rendered, plan)
	if err != nil {
		return fmt.Errorf("append source: %w", err)
	}
	if err := writeOutput(outputPath, withAppendix); err != nil {
		return fmt.Errorf("write: %w", err)
	}
	return nil
}

// codeFence returns the shortest backtick fence that can safely wrap code,
// always at least three backticks long. A run of N backticks inside code
// requires a fence of at least N+1 backticks to prevent premature closure
// in markdown renderers.
func codeFence(code string) string {
	longest := 0
	cur := 0
	for _, r := range code {
		if r == '`' {
			cur++
			if cur > longest {
				longest = cur
			}
		} else {
			cur = 0
		}
	}
	n := longest + 1
	if n < 3 {
		n = 3
	}
	return strings.Repeat("`", n)
}

func buildSchemaJSON() (string, error) {
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
				"current validator requires non-empty title, overview, definition_of_done.narrative, current_state, and module_shape",
				"current validator requires at least one goal and at least one implementation step",
				"current validator requires each implementation step to include title, summary, and at least one file change",
				"current validator requires each file change to include filename, explanation, language, and code",
				"current validator requires the top-level verification field to be present",
				"verification.summary, verification.automated, and verification.manual remain optional",
			},
		},
	}

	raw, err := json.MarshalIndent(schema, "", "  ")
	if err != nil {
		return "", fmt.Errorf("marshal schema: %w", err)
	}
	return string(raw), nil
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
