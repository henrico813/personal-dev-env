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

	if exitCode := run([]string{"show-schema"}, &stdout, &stderr); exitCode != 0 {
		t.Fatalf("run(show-schema) exit code = %d, want 0, stderr = %q", exitCode, stderr.String())
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

func TestBuildHelpTextExplainsPlanExampleInputAndRules(t *testing.T) {
	help := buildHelpText()

	requiredSnippets := []string{
		"Prints a JSON object with plan_example and validation_rules.",
		"Use only plan_example as input to planner validate and planner create.",
		"definition_of_done.goals must contain between 1 and 8 items",
		"each definition_of_done.goals item must be at most 88 characters",
	}
	for _, snippet := range requiredSnippets {
		if !strings.Contains(help, snippet) {
			t.Fatalf("buildHelpText() missing %q", snippet)
		}
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
