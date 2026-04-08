package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"regexp"
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
	return validateSteps(plan.Implementation)
}

// validateSteps checks the per-step invariants so mutation paths can validate
// a []Step slice without constructing a full Plan.
func validateSteps(steps []Step) error {
	if len(steps) == 0 {
		return errors.New("at least one implementation step is required")
	}
	for _, step := range steps {
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

// decodeSteps unmarshals a JSON array of Steps from the incoming steps.json file.
func decodeSteps(data []byte) ([]Step, error) {
	var steps []Step
	if err := json.Unmarshal(data, &steps); err != nil {
		return nil, err
	}
	return steps, nil
}

// parseImplementationSection locates the ## Implementation section in md,
// delegates to parseSteps, and returns the byte range [start, end) so the
// caller can splice in a replacement section. start is the index of '##' in
// "## Implementation"; end is the index of the next "\n\n## " heading or
// len(md) if this is the last section.
func parseImplementationSection(md string) (steps []Step, start, end int, err error) {
	const heading = "## Implementation"
	if strings.HasPrefix(md, heading) {
		start = 0
	} else {
		idx := strings.Index(md, "\n"+heading)
		if idx == -1 {
			return nil, 0, 0, errors.New("## Implementation section not found")
		}
		start = idx + 1
	}
	afterHeading := start + len(heading)
	end = len(md)
	if idx := strings.Index(md[afterHeading:], "\n\n## "); idx != -1 {
		end = afterHeading + idx
	}
	steps, err = parseSteps(md[start:end])
	return steps, start, end, err
}

// parseSteps is the inverse of renderImplementationSection. It walks the
// section line by line using five token types emitted by the template:
//   - "### N. Title"    — step heading
//   - "`filename`"      — file change filename
//   - "> explanation"   — file change explanation
//   - fence+lang line   — opening code fence (3+ backticks + language)
//   - exact fence line  — closing code fence
//
// Lines before the first filename accumulate as the step summary. Lines
// between fence-open and fence-close accumulate as FileChange.Code. Blank
// lines between explanation and fence, and between file changes, are skipped.
func parseSteps(section string) ([]Step, error) {
	stepRE     := regexp.MustCompile(`^### \d+\. (.+)$`)
	filenameRE := regexp.MustCompile("^`([^`]+)`$")
	explRE     := regexp.MustCompile(`^> (.+)$`)
	fenceRE    := regexp.MustCompile("^(`{3,})(\\w*)$")

	var steps []Step
	var step *Step
	var fc *FileChange
	var summaryLines, codeLines []string
	var fence string
	inCode := false

	flushFC := func() {
		if fc != nil && step != nil {
			fc.Code = strings.Join(codeLines, "\n")
			step.FileChanges = append(step.FileChanges, *fc)
			fc = nil
			codeLines = nil
		}
	}
	flushStep := func() {
		if step != nil {
			step.Summary = strings.Trim(strings.Join(summaryLines, "\n"), "\n")
			steps = append(steps, *step)
			step = nil
			summaryLines = nil
		}
	}

	for _, line := range strings.Split(section, "\n") {
		if inCode {
			if line == fence {
				inCode = false
				fence = ""
				flushFC()
			} else {
				codeLines = append(codeLines, line)
			}
			continue
		}
		if m := stepRE.FindStringSubmatch(line); m != nil {
			flushStep()
			step = &Step{Title: m[1]}
			continue
		}
		if step == nil {
			continue
		}
		if fc != nil {
			if fc.Explanation == "" {
				if m := explRE.FindStringSubmatch(line); m != nil {
					fc.Explanation = m[1]
				}
			} else if m := fenceRE.FindStringSubmatch(line); m != nil {
				fence = m[1]
				fc.Language = m[2]
				inCode = true
				codeLines = nil
			}
			// blank lines between explanation and fence are skipped
			continue
		}
		if m := filenameRE.FindStringSubmatch(line); m != nil {
			fc = &FileChange{Filename: m[1]}
			continue
		}
		// only accumulate summary before the first file change
		if len(step.FileChanges) == 0 {
			summaryLines = append(summaryLines, line)
		}
		// blank lines between file changes are skipped
	}
	flushStep()
	return steps, nil
}

// renderImplementationSection produces the same bytes the main template emits
// for the ## Implementation section. It uses codeFence so multi-backtick Code
// values render with a fence long enough to prevent premature closure.
// The output is paired with parseSteps via TestRenderParseRoundTrip — any
// drift between the two is caught at test time.
func renderImplementationSection(steps []Step) string {
	var sb strings.Builder
	sb.WriteString("## Implementation\n---")
	for i, step := range steps {
		fmt.Fprintf(&sb, "\n\n### %d. %s\n\n%s", i+1, step.Title, step.Summary)
		for _, fc := range step.FileChanges {
			fence := codeFence(fc.Code)
			fmt.Fprintf(&sb, "\n\n`%s`\n> %s\n\n%s%s\n%s\n%s",
				fc.Filename, fc.Explanation, fence, fc.Language, fc.Code, fence)
		}
	}
	return sb.String()
}

// createPlanFromStruct validates, renders, and atomically writes a plan.
// Used by both planner create and the step mutation commands to ensure
// the same validation and render path is always exercised.
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
	if err := writeOutput(outputPath, rendered); err != nil {
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
				"help":                "Print built-in usage and workflow guidance.",
				"show-schema":         "Print this full contract, including nested JSON shape, current validator rules, and command semantics.",
				"validate":            "Validate planner JSON input without rendering markdown. Usage: planner validate <plan.json>.",
				"create":              "Render canonical markdown from valid planner JSON. Usage: planner create <plan.json> <output.md>.",
				"create step add":     "Append steps to an existing plan file. Usage: planner create step add <steps.json> <plan.md>.",
				"create step replace": "Replace all implementation steps in an existing plan file. Usage: planner create step replace <steps.json> <plan.md>.",
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
