package main

import (
	"bytes"
	_ "embed"
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"text/template"
)

//go:embed plan_template.md.tmpl
var planTemplate string

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

// createPlanFromStruct validates, renders, and atomically writes canonical
// markdown. Rendered plans are markdown-only outputs and do not embed JSON
// appendices.
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
