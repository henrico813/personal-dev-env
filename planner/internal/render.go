package internal

import (
	"bytes"
	_ "embed"
	"fmt"
	"os"
	"path/filepath"
	"text/template"
)

//go:embed plan_template.md.tmpl
var planTemplate string

//go:embed implementation_section.md.tmpl
var implementationSectionTemplate string

//go:embed implementation_step.md.tmpl
var implementationStepTemplate string

func CreatePlan(inputPath string, outputPath string) error {
	plan, err := ReadPlanFile(inputPath)
	if err != nil {
		return fmt.Errorf("%s: %w", inputPath, err)
	}
	if err := CreatePlanFromStruct(plan, outputPath); err != nil {
		return fmt.Errorf("%s: %w", inputPath, err)
	}
	return nil
}

// CreatePlanFromStruct validates, renders, and atomically writes canonical
// markdown, preserving any existing leading frontmatter on rewrite. The CLI
// entrypoints share this path so scaffold rendering and rewrite preservation
// stay aligned in one place.
func CreatePlanFromStruct(plan Plan, outputPath string) error {
	if err := ValidatePlan(plan); err != nil {
		return fmt.Errorf("validate: %w", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		return fmt.Errorf("render: %w", err)
	}
	if err := VerifyRenderedText(rendered, plan); err != nil {
		return fmt.Errorf("verify: %w", err)
	}
	finalRendered, err := preserveExistingFrontmatter(outputPath, rendered)
	if err != nil {
		return fmt.Errorf("frontmatter: %w", err)
	}
	if err := writeOutput(outputPath, finalRendered); err != nil {
		return fmt.Errorf("write: %w", err)
	}
	return nil
}

func renderCanonicalScaffold() (string, error) {
	plan := BuildPlanTemplate()
	if err := ValidatePlan(plan); err != nil {
		return "", fmt.Errorf("validate: %w", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		return "", fmt.Errorf("render: %w", err)
	}
	if err := VerifyRenderedText(rendered, plan); err != nil {
		return "", fmt.Errorf("verify: %w", err)
	}
	return rendered, nil
}

// RenderPlan renders a validated Plan to canonical markdown format.
func RenderPlan(plan Plan) (string, error) {
	tmpl, err := template.New("plan_template.md.tmpl").Funcs(template.FuncMap{
		"inc":          func(i int) int { return i + 1 },
		"getCodeFence": GetCodeFence,
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

// preserveExistingFrontmatter keeps supported frontmatter on rewrite so the
// CLI can preserve it without changing markdown decode error handling.
func preserveExistingFrontmatter(outputPath, rendered string) (string, error) {
	info, err := os.Stat(outputPath)
	if err != nil {
		if os.IsNotExist(err) {
			return rendered, nil
		}
		return "", err
	}
	if !info.Mode().IsRegular() {
		return rendered, nil
	}
	raw, err := os.ReadFile(outputPath)
	if err != nil {
		return "", err
	}
	frontmatter, _, err := splitFrontmatter(string(raw))
	if err != nil {
		return "", err
	}
	if frontmatter == "" {
		return rendered, nil
	}
	return frontmatter + rendered, nil
}

// writeOutput atomically writes rendered content to path via a temp file and
// os.Rename so a failed or interrupted write cannot corrupt the destination.
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

// RenderImplementationSection renders the full Implementation section for a plan.
func RenderImplementationSection(steps []Step) (string, error) {
	wrapped := Plan{Implementation: steps}
	tmpl, err := template.New("implementation_section").Funcs(template.FuncMap{
		"inc":          func(i int) int { return i + 1 },
		"getCodeFence": GetCodeFence,
	}).Parse(implementationSectionTemplate)
	if err != nil {
		return "", err
	}
	var buf bytes.Buffer
	if err := tmpl.Execute(&buf, wrapped); err != nil {
		return "", err
	}
	return buf.String(), nil
}

// RenderImplementationStep renders a single implementation step at the given index.
func RenderImplementationStep(index int, step Step) (string, error) {
	tmpl, err := template.New("implementation_step").Funcs(template.FuncMap{
		"getCodeFence": GetCodeFence,
	}).Parse(implementationStepTemplate)
	if err != nil {
		return "", err
	}
	var buf bytes.Buffer
	data := struct {
		Index int
		Step  Step
	}{Index: index, Step: step}
	if err := tmpl.Execute(&buf, data); err != nil {
		return "", err
	}
	return buf.String(), nil
}
