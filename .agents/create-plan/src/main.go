package main

import (
	"bytes"
	_ "embed"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
	"text/template"
)

//go:embed plan_template.md.tmpl
var planTemplate string

const helpText = `planner generates implementation-plan markdown from canonical JSON.

Usage:
  planner
  planner help
  planner show-schema
  planner validate <plan.json>
  planner create <input.json> <output.md>
  planner create step add <steps.json> <plan.md>
  planner create step replace <steps.json> <plan.md>

Scratch flow:
  1. Research the task.
  2. Run planner show-schema.
  3. Write plan JSON that matches planner show-schema.
  4. Run planner validate <plan.json>.
  5. Run planner create <plan.json> <output.md>.

Rewrite flow (partial edit):
  1. Read the existing rendered plan file.
  2. Write a JSON array of new steps matching the implementation step schema.
  3. Run planner create step add <steps.json> <plan.md>   (append steps).
     Or: planner create step replace <steps.json> <plan.md> (replace all steps).
  4. Verify the rewritten plan file.

Rewrite flow (full rewrite):
  1. Read the existing markdown issue.
  2. Map its content into canonical JSON matching planner show-schema.
  3. Run planner validate <plan.json>.
  4. Run planner create <plan.json> <output.md>.
  5. Compare the rendered issue with the source issue for dropped content.

show-schema contract:
  - Includes the nested JSON shape the current validator recognizes.
  - Includes the required fields and constraints the current validator enforces.
  - Includes command semantics for help, validate, and create.
`

func main() {
	os.Exit(run(os.Args[1:], os.Stdout, os.Stderr))
}

func run(args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) == 0 {
		printHelp(stdout)
		return 0
	}

	switch args[0] {
	case "help", "--help", "-h":
		printHelp(stdout)
		return 0
	case "show-schema":
		return runShowSchema(stdout, stderr)
	case "validate":
		return runValidate(args[1:], stdout, stderr)
	case "create":
		return runCreate(args[1:], stdout, stderr)
	default:
		fmt.Fprintf(stderr, "unknown command: %s\n\n", args[0])
		printHelp(stderr)
		return 2
	}
}

func printHelp(w io.Writer) {
	_, _ = io.WriteString(w, helpText)
}

func runShowSchema(stdout io.Writer, stderr io.Writer) int {
	schemaJSON, err := buildSchemaJSON()
	if err != nil {
		fmt.Fprintf(stderr, "build schema: %v\n", err)
		return 1
	}
	if _, err := io.WriteString(stdout, schemaJSON+"\n"); err != nil {
		fmt.Fprintf(stderr, "write schema: %v\n", err)
		return 1
	}
	return 0
}

func runValidate(args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) != 1 {
		fmt.Fprintln(stderr, "usage: planner validate <plan.json>")
		return 2
	}
	plan, err := readPlanFile(args[0])
	if err != nil {
		fmt.Fprintf(stderr, "validate %s: %v\n", args[0], err)
		return 1
	}
	if err := validatePlan(plan); err != nil {
		fmt.Fprintf(stderr, "validate %s: %v\n", args[0], err)
		return 1
	}
	_, _ = io.WriteString(stdout, "OK\n")
	return 0
}

func runCreate(args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) >= 1 && args[0] == "step" {
		return runStep(args[1:], stdout, stderr)
	}
	if len(args) != 2 {
		fmt.Fprintln(stderr, "usage: planner create <plan.json> <output.md>")
		return 2
	}
	if err := createPlan(args[0], args[1]); err != nil {
		fmt.Fprintf(stderr, "create: %v\n", err)
		return 1
	}
	_, _ = io.WriteString(stdout, args[1]+"\n")
	return 0
}

func runStep(args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) < 1 {
		fmt.Fprintln(stderr, "usage: planner create step <add|replace> <steps.json> <plan.md>")
		return 2
	}
	switch args[0] {
	case "add":
		return runStepAdd(args[1:], stdout, stderr)
	case "replace":
		return runStepReplace(args[1:], stdout, stderr)
	default:
		fmt.Fprintf(stderr, "unknown step command: %s\n", args[0])
		return 2
	}
}

// mutatePlan parses the ## Implementation section of planPath into []Step,
// applies fn to produce the mutated slice, validates it, re-renders only the
// implementation section via renderImplementationSection, splices it back into
// the file, and writes atomically. The verb label prefixes error messages.
func mutatePlan(verb, stepsPath, planPath string, fn func(existing, incoming []Step) []Step) error {
	mdBytes, err := os.ReadFile(planPath)
	if err != nil {
		return fmt.Errorf("%s: read %s: %w", verb, planPath, err)
	}
	md := string(mdBytes)
	existing, start, end, err := parseImplementationSection(md)
	if err != nil {
		return fmt.Errorf("%s: parse implementation section from %s: %w", verb, planPath, err)
	}
	stepsData, err := os.ReadFile(stepsPath)
	if err != nil {
		return fmt.Errorf("%s: read %s: %w", verb, stepsPath, err)
	}
	incoming, err := decodeSteps(stepsData)
	if err != nil {
		return fmt.Errorf("%s: decode steps from %s: %w", verb, stepsPath, err)
	}
	if len(incoming) == 0 {
		return fmt.Errorf("%s: steps.json must contain at least one step", verb)
	}
	mutated := fn(existing, incoming)
	if err := validateSteps(mutated); err != nil {
		return fmt.Errorf("%s: validate: %w", verb, err)
	}
	newSection := renderImplementationSection(mutated)
	if err := writeOutput(planPath, md[:start]+newSection+md[end:]); err != nil {
		return fmt.Errorf("%s: write: %w", verb, err)
	}
	return nil
}

func runStepAdd(args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) != 2 {
		fmt.Fprintln(stderr, "usage: planner create step add <steps.json> <plan.md>")
		return 2
	}
	if err := mutatePlan("step add", args[0], args[1], func(existing, incoming []Step) []Step {
		return append(existing, incoming...)
	}); err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	_, _ = io.WriteString(stdout, args[1]+"\n")
	return 0
}

func runStepReplace(args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) != 2 {
		fmt.Fprintln(stderr, "usage: planner create step replace <steps.json> <plan.md>")
		return 2
	}
	if err := mutatePlan("step replace", args[0], args[1], func(_, incoming []Step) []Step {
		return incoming
	}); err != nil {
		fmt.Fprintln(stderr, err)
		return 1
	}
	_, _ = io.WriteString(stdout, args[1]+"\n")
	return 0
}

func createPlan(inputPath string, outputPath string) error {
	plan, err := readPlanFile(inputPath)
	if err != nil {
		return fmt.Errorf("%s: %w", inputPath, err)
	}
	if err := createPlanFromStruct(plan, outputPath); err != nil {
		return fmt.Errorf("%s: %w", inputPath, err)
	}
	return nil
}

func readPlanFile(path string) (Plan, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return Plan{}, err
	}
	return decodePlan(data)
}

func renderPlan(plan Plan) (string, error) {
	tmpl, err := template.New("plan_template.md.tmpl").Funcs(template.FuncMap{
		"inc":       func(i int) int { return i + 1 },
		"codeFence": codeFence,
	}).Parse(planTemplate)
	if err != nil {
		return "", err
	}

	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, plan); err != nil {
		return "", err
	}
	return buf.String(), nil
}

func verifyRenderedText(rendered string, plan Plan) error {
	requiredSections := []string{
		"## Overview",
		"## Definition of Done",
		"### Current State",
		"### Module Shape",
		"## Implementation",
		"## Verification",
	}

	for _, section := range requiredSections {
		if !strings.Contains(rendered, section) {
			return fmt.Errorf("missing section: %s", section)
		}
	}

	if !strings.Contains(rendered, "### 1.") {
		return errors.New("missing numbered implementation step")
	}

	for i, step := range plan.Implementation {
		heading := fmt.Sprintf("### %d. %s", i+1, step.Title)
		if !strings.Contains(rendered, heading) {
			return fmt.Errorf("missing rendered implementation step: %s", heading)
		}
		for _, change := range step.FileChanges {
			fence := codeFence(change.Code)
			block := fence + change.Language + "\n" + change.Code + "\n" + fence
			if !strings.Contains(rendered, block) {
				return fmt.Errorf("missing rendered code block for %s", change.Filename)
			}
		}
	}

	return nil
}

// writeOutput atomically writes rendered content to path via a temp file and
// os.Rename so a failed or interrupted write cannot corrupt the existing plan
// file. This is essential for step add/replace, which overwrite the only copy.
func writeOutput(path, rendered string) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(filepath.Dir(path), ".planner-*.md.tmp")
	if err != nil {
		return err
	}
	tmpPath := tmp.Name()
	cleanup := func() { _ = os.Remove(tmpPath) }
	if _, err := tmp.WriteString(rendered); err != nil {
		_ = tmp.Close()
		cleanup()
		return err
	}
	if err := tmp.Sync(); err != nil {
		_ = tmp.Close()
		cleanup()
		return err
	}
	if err := tmp.Close(); err != nil {
		cleanup()
		return err
	}
	if err := os.Rename(tmpPath, path); err != nil {
		cleanup()
		return err
	}
	return nil
}
