package main

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"

	"planner/schema"
)

func TestRunShowSchemaPrintsPlanExampleAndValidationRules(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"show-schema"}, &stdout, &stderr); exitCode != 0 {
		t.Fatalf("Execute(show-schema) exit code = %d, want 0, stderr = %q", exitCode, stderr.String())
	}

	var doc schema.SchemaDocument
	if err := json.Unmarshal(stdout.Bytes(), &doc); err != nil {
		t.Fatalf("show-schema output is not valid schema document JSON: %v", err)
	}

	if doc.PlanExample.Title == "" {
		t.Fatal("show-schema output missing plan_example")
	}
	if len(doc.ValidationRules) == 0 {
		t.Fatal("show-schema output missing validation_rules")
	}

	wantRules := []string{
		"definition_of_done.goals must contain between 1 and 8 items",
		"each definition_of_done.goals item must be at most 88 characters",
	}
	for _, want := range wantRules {
		if !contains(doc.ValidationRules, want) {
			t.Fatalf("show-schema validation_rules missing %q", want)
		}
	}
}

func TestHelpTextExplainsPlanExampleInputAndRules(t *testing.T) {
	help := buildHelpText()

	requiredSnippets := []string{
		"Prints a JSON object with plan_example and validation_rules.",
		"Use only plan_example as input to planner validate and planner create.",
		"planner inspect <plan.md>",
		"planner replace <plan.md> <patch.json> <output.md> --section <section>",
		"--subsection <name-or-index>",
		"--append",
		"definition_of_done.goals must contain between 1 and 8 items",
		"each definition_of_done.goals item must be at most 88 characters",
	}
	for _, snippet := range requiredSnippets {
		if !strings.Contains(help, snippet) {
			t.Fatalf("buildHelpText() missing %q", snippet)
		}
	}
}

func TestRunInspectUsage(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"inspect"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("Execute(inspect) exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "usage: planner inspect <plan.md>") {
		t.Fatalf("missing inspect usage in stderr = %q", stderr.String())
	}
}

func TestRunReplaceUsage(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"replace"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("Execute(replace) exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "usage: planner replace <plan.md> <patch.json> <output.md> --section <section>") {
		t.Fatalf("missing replace usage in stderr = %q", stderr.String())
	}
}

func TestExecuteReplaceRejectsUnknownFlag(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	if exitCode := Execute([]string{"replace", "a.md", "b.json", "c.md", "--unknown"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "unknown flag") {
		t.Fatalf("expected unknown flag error, got %q", stderr.String())
	}
}

func TestExecuteReplaceRequiresSection(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	if exitCode := Execute([]string{"replace", "a.md", "b.json", "c.md"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "--section is required") {
		t.Fatalf("expected required section error, got %q", stderr.String())
	}
}

func contains(values []string, want string) bool {
	for _, value := range values {
		if value == want {
			return true
		}
	}
	return false
}
