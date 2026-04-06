package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"text/template"
)

const templateRelativePath = "../plan_template.md.tmpl"

func main() {
	if len(os.Args) != 3 {
		fmt.Fprintln(os.Stderr, "usage: create-plan-engine <input.json> <output.md>")
		os.Exit(1)
	}

	tmpl := readTemplate()
	plan := readInput(os.Args[1])
	validatePlan(plan)
	rendered := writeText(tmpl, plan)
	verifyText(rendered, plan)
	writeOutput(os.Args[2], rendered)
}

func readTemplate() *template.Template {
	exePath, err := os.Executable()
	if err != nil {
		panic(err)
	}

	candidates := []string{
		filepath.Clean(filepath.Join(filepath.Dir(exePath), templateRelativePath)),
		filepath.Clean(filepath.Join("..", "plan_template.md.tmpl")),
	}

	var tmplPath string
	for _, candidate := range candidates {
		if _, err := os.Stat(candidate); err == nil {
			tmplPath = candidate
			break
		}
	}

	if tmplPath == "" {
		panic("plan template not found")
	}

	funcs := template.FuncMap{
		"inc": func(i int) int { return i + 1 },
	}

	return template.Must(
		template.New(filepath.Base(tmplPath)).
			Funcs(funcs).
			ParseFiles(tmplPath),
	)
}

func readInput(path string) Plan {
	data, err := os.ReadFile(path)
	if err != nil {
		panic(err)
	}

	var plan Plan
	if err := json.Unmarshal(data, &plan); err != nil {
		panic(err)
	}

	return plan
}

func writeText(tmpl *template.Template, plan Plan) string {
	var buf bytes.Buffer

	if err := tmpl.Execute(&buf, plan); err != nil {
		panic(err)
	}

	return buf.String()
}

func verifyText(rendered string, plan Plan) {
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
			panic(fmt.Sprintf("missing section: %s", section))
		}
	}

	if !strings.Contains(rendered, "### 1.") {
		panic("missing numbered implementation step")
	}

	for _, step := range plan.Implementation {
		if !strings.Contains(rendered, "### "+step.Title) && !strings.Contains(rendered, ". "+step.Title) {
			panic(fmt.Sprintf("missing rendered implementation step: %s", step.Title))
		}

		for _, change := range step.FileChanges {
			fence := "```" + change.Language + "\n" + change.Code + "\n```"
			if !strings.Contains(rendered, fence) {
				panic(fmt.Sprintf("missing rendered code block for %s", change.Filename))
			}
		}
	}
}

func writeOutput(path, rendered string) {
	if err := os.WriteFile(path, []byte(rendered), 0o644); err != nil {
		panic(err)
	}
}
