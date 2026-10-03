package internal

import (
	"bytes"
	_ "embed"
	"fmt"
	"text/template"
	"time"
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

// renderIssueScaffold prepends the vault issue frontmatter Planner accepts to
// the default scaffold. date supplies date_created so callers can pass the
// current day and tests can pin it. The combined document is validated through
// splitMarkdownEnvelope, so planner new fails instead of writing a document
// that check would later reject as a wrapped issue doc.
func renderIssueScaffold(project string, date time.Time) (string, error) {
	body, err := renderCanonicalScaffold()
	if err != nil {
		return "", err
	}
	document := fmt.Sprintf(
		"---\ntags:\n  - \"#Ticket\"\ntype: issue\nstatus: open\ntemplate_version: 1\nproject: %s\ndate_created: %s\ntopics: []\n---\n\n%s",
		project, date.Format("2006-01-02"), body)
	if _, err := splitMarkdownEnvelope(document); err != nil {
		return "", fmt.Errorf("issue frontmatter: %w", err)
	}
	return document, nil
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
