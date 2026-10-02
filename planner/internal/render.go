package internal

import (
	"bytes"
	_ "embed"
	"fmt"
	"text/template"
)

//go:embed plan_template.md.tmpl
var planTemplate string

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
