package main

import (
	"bytes"
	"encoding/json"
	"os"
	"strings"
	"testing"

	"planner/schema"
)

func TestShowSchemaIncludesRules(t *testing.T) {
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
		"definition_of_done checklist items must have non-empty text; object status may be pending or done",
		"verification checklist items must have non-empty text; object status may be pending or done",
		"each definition_of_done.goals item must be at most 88 characters",
	}
	for _, want := range wantRules {
		if !contains(doc.ValidationRules, want) {
			t.Fatalf("show-schema validation_rules missing %q", want)
		}
	}
}

func TestHelpTextIncludesRules(t *testing.T) {
	help := buildHelpText()

	requiredSnippets := []string{
		"Prints a JSON object with plan_example and validation_rules.",
		"Use only plan_example as input to planner validate and planner create.",
		"planner inspect <plan.md>",
		"planner replace <plan.md> [<patch.json>] <output.md> --section <section> [--subsection <name-or-index>] [--append] [--stdin] [--diff] [--write]",
		"--subsection <name-or-index>",
		"--append",
		"definition_of_done.goals must contain between 1 and 8 items",
		"definition_of_done checklist items must have non-empty text; object status may be pending or done",
		"verification checklist items must have non-empty text; object status may be pending or done",
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
	if !strings.Contains(stderr.String(), "usage: planner replace <plan.md> [<patch.json>] <output.md> --section <section>") {
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

func TestCreateReadsStdinWhenFlagSet(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
	})
	if _, err := os.Stat(out); err != nil {
		t.Fatalf("expected output: %v", err)
	}
}

func TestCreateAutoDetectsPipedStdin(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
	})
	if _, err := os.Stat(out); err != nil {
		t.Fatalf("expected output: %v", err)
	}
}

func TestDiffAloneExitsOneOnChange(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	if err := os.WriteFile(out, []byte("stale\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin", "--diff"}, &stdout, &stderr); exit != 1 {
			t.Fatalf("exit %d want 1; stderr %q", exit, stderr.String())
		}
		if stdout.Len() == 0 {
			t.Fatal("want non-empty diff on stdout")
		}
	})
	data, _ := os.ReadFile(out)
	if string(data) != "stale\n" {
		t.Fatalf("file must be unchanged: %q", string(data))
	}
}

func TestDiffAndWriteExitsZero(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin", "--diff", "--write"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
	})
	if _, err := os.Stat(out); err != nil {
		t.Fatalf("expected output: %v", err)
	}
}

func TestWriteOnlyPrintsOutputPath(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin", "--write"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
		if !strings.Contains(stdout.String(), out) {
			t.Fatalf("stdout missing output path; got %q", stdout.String())
		}
	})
}

func withStdin(t *testing.T, data []byte, fn func()) {
	t.Helper()
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatalf("pipe: %v", err)
	}
	original := os.Stdin
	os.Stdin = r
	defer func() { os.Stdin = original }()
	go func() { defer w.Close(); _, _ = w.Write(data) }()
	fn()
}

func TestReplaceWriteFailureEmitsNoResult(t *testing.T) {
	dir := t.TempDir()
	// Write source plan to disk.
	planPath := dir + "/plan.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", planPath, "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("create exit %d stderr %q", exit, stderr.String())
		}
	})
	// Write patch to disk.
	patchPath := dir + "/patch.json"
	if err := os.WriteFile(patchPath, []byte(`"Updated overview"`), 0o644); err != nil {
		t.Fatal(err)
	}
	// Make output directory read-only so WriteAtomic fails.
	roDir := dir + "/readonly"
	if err := os.Mkdir(roDir, 0o555); err != nil {
		t.Fatal(err)
	}
	outputPath := roDir + "/out.md"

	var stdout, stderr bytes.Buffer
	exit := Execute([]string{"replace", planPath, patchPath, outputPath, "--section", "overview", "--write"}, &stdout, &stderr)
	if exit == 0 {
		t.Fatalf("expected non-zero exit on write failure, got 0")
	}
	if strings.Contains(stdout.String(), "{") {
		t.Fatalf("replace result JSON must not be emitted on write failure; stdout = %q", stdout.String())
	}
}

func TestValidateSurfacesSchemaErrors(t *testing.T) {
	tests := []struct {
		name    string
		input   func(t *testing.T) []byte
		wantErr string
	}{
		{
			name:    "trailing_data",
			input:   func(t *testing.T) []byte { return append(validPlanJSON(), []byte(" trailing")...) },
			wantErr: "trailing data after plan JSON",
		},
		{
			name: "unknown_field",
			input: func(t *testing.T) []byte {
				return validPlanJSONMap(t, map[string]any{"extra_field": "boom"})
			},
			wantErr: "unknown field",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			path := dir + "/plan.json"
			if err := os.WriteFile(path, tc.input(t), 0o644); err != nil {
				t.Fatal(err)
			}

			assertCommandError(t, []string{"validate", path}, "validate:", tc.wantErr, 1)
		})
	}
}

func TestCreateSurfacesSchemaErrors(t *testing.T) {
	tests := []struct {
		name    string
		input   func(t *testing.T) []byte
		wantErr string
	}{
		{
			name:    "trailing_data",
			input:   func(t *testing.T) []byte { return append(validPlanJSON(), []byte(" trailing")...) },
			wantErr: "trailing data after plan JSON",
		},
		{
			name: "unknown_field",
			input: func(t *testing.T) []byte {
				return validPlanJSONMap(t, map[string]any{"extra_field": "boom"})
			},
			wantErr: "unknown field",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			dir := t.TempDir()
			inputPath := dir + "/plan.json"
			outputPath := dir + "/out.md"
			if err := os.WriteFile(inputPath, tc.input(t), 0o644); err != nil {
				t.Fatal(err)
			}

			assertCommandError(t, []string{"create", inputPath, outputPath}, "create:", tc.wantErr, 1)
			if _, err := os.Stat(outputPath); !os.IsNotExist(err) {
				t.Fatalf("output should not be written on validation error, stat err = %v", err)
			}
		})
	}
}

func assertCommandError(t *testing.T, args []string, wantPrefix, wantErr string, wantExit int) {
	t.Helper()
	var stdout, stderr bytes.Buffer
	exit := Execute(args, &stdout, &stderr)
	if exit != wantExit {
		t.Fatalf("exit = %d, want %d; stderr = %q", exit, wantExit, stderr.String())
	}
	if !strings.Contains(stderr.String(), wantPrefix) {
		t.Fatalf("stderr missing command prefix %q: %q", wantPrefix, stderr.String())
	}
	if !strings.Contains(stderr.String(), wantErr) {
		t.Fatalf("stderr missing schema error %q: %q", wantErr, stderr.String())
	}
}

func validPlanJSON() []byte {
	return mustJSON(schema.Plan{
		Title:    "T",
		Overview: "O",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "N",
			Goals:        []schema.ChecklistItem{{Text: "g"}},
			CurrentState: "C",
			ModuleShape:  "M",
		},
		Implementation: []schema.Step{{
			Title:   "T",
			Summary: "S",
			FileChanges: []schema.FileChange{{
				Filename:    "f",
				Explanation: "e",
				Diff:        "@@ -1 +1 @@\n-a\n+b",
			}},
		}},
		Verification: &schema.Verification{
			Summary:   "",
			Automated: []schema.ChecklistItem{{Text: "A"}},
			Manual:    []schema.ChecklistItem{{Text: "M"}},
		},
	})
}

func validPlanJSONMap(t *testing.T, kv map[string]any) []byte {
	t.Helper()
	var doc map[string]any
	if err := json.Unmarshal(validPlanJSON(), &doc); err != nil {
		t.Fatalf("json.Unmarshal: %v", err)
	}
	for key, value := range kv {
		doc[key] = value
	}
	return mustJSON(doc)
}

func mustJSON(v any) []byte {
	raw, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	return raw
}

func TestReplaceReadsStdinPatch(t *testing.T) {
	dir := t.TempDir()
	src := dir + "/plan.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", src, "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("seed exit %d stderr %q", exit, stderr.String())
		}
	})
	patch := []byte(`"Updated overview text."`)
	withStdin(t, patch, func() {
		var stdout, stderr bytes.Buffer
		exit := Execute([]string{"replace", src, src, "--section", "overview", "--stdin", "--write"}, &stdout, &stderr)
		if exit != 0 {
			t.Fatalf("replace exit %d stderr %q", exit, stderr.String())
		}
	})
	data, err := os.ReadFile(src)
	if err != nil {
		t.Fatalf("read: %v", err)
	}
	if !strings.Contains(string(data), "Updated overview text.") {
		t.Fatalf("replaced overview not present:\n%s", string(data))
	}
}

func TestDiffAndWriteDoesNotEmitResult(t *testing.T) {
	dir := t.TempDir()
	src := dir + "/plan.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", src, "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("seed exit %d stderr %q", exit, stderr.String())
		}
	})
	patch := []byte(`"Fresh overview text."`)
	withStdin(t, patch, func() {
		var stdout, stderr bytes.Buffer
		exit := Execute([]string{"replace", src, src, "--section", "overview", "--stdin", "--diff", "--write"}, &stdout, &stderr)
		if exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
		if strings.Contains(stdout.String(), `"section"`) {
			t.Fatalf("replace result JSON must not appear on stdout when --diff is set; stdout = %q", stdout.String())
		}
	})
}
