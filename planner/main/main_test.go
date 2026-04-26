package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"os"
	"strings"
	"testing"

	"planner/schema"
	"planner/validate"
)

func TestHelpTextIncludesRules(t *testing.T) {
	help := buildHelpText()

	// Positive anchors: every command we ship must appear in help so AIs can
	// discover the current surface from `planner help` alone.
	for _, command := range []string{
		"planner template",
		"planner validate",
		"planner create",
		"planner inspect",
		"planner replace",
	} {
		if !strings.Contains(help, command) {
			t.Fatalf("buildHelpText() missing command %q", command)
		}
	}

	// Negative anchors: deleted commands and removed flags must not reappear.
	for _, banned := range []string{"show-schema", "planner generate", "--write"} {
		if strings.Contains(help, banned) {
			t.Fatalf("buildHelpText() still mentions removed token %q", banned)
		}
	}
}

func TestTemplateMarkdownIncludesPlaceholder(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"template", "--md"}, &stdout, &stderr); exitCode != 0 {
		t.Fatalf("Execute(template --md) exit code = %d, want 0, stderr = %q", exitCode, stderr.String())
	}
	if !strings.Contains(stdout.String(), "PLACEHOLDER") {
		t.Fatalf("template markdown output missing PLACEHOLDER: %q", stdout.String())
	}
}

func TestTemplateJSONIsValidPlan(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"template", "--json"}, &stdout, &stderr); exitCode != 0 {
		t.Fatalf("Execute(template --json) exit code = %d, want 0, stderr = %q", exitCode, stderr.String())
	}

	plan, err := schema.DecodePlan(stdout.Bytes())
	if err != nil {
		t.Fatalf("template JSON output is not valid plan JSON: %v", err)
	}
	if err := validate.ValidatePlan(plan); err != nil {
		t.Fatalf("template JSON plan does not validate: %v", err)
	}
	if plan.Implementation[0].FileChanges[0].Diff != "PLACEHOLDER" {
		t.Fatalf("template diff = %q, want PLACEHOLDER", plan.Implementation[0].FileChanges[0].Diff)
	}
}

func TestTemplateSectionShapes(t *testing.T) {
	tests := []struct {
		name  string
		args  []string
		check func(*testing.T, []byte)
	}{
		{
			name: "overview",
			args: []string{"template", "--json", "--section", "overview"},
			check: func(t *testing.T, raw []byte) {
				t.Helper()
				var got string
				if err := json.Unmarshal(raw, &got); err != nil {
					t.Fatalf("overview output not string JSON: %v", err)
				}
				if got == "" {
					t.Fatal("overview is empty")
				}
			},
		},
		{
			name: "definition_of_done",
			args: []string{"template", "--json", "--section", "definition_of_done"},
			check: func(t *testing.T, raw []byte) {
				t.Helper()
				var got map[string]any
				if err := json.Unmarshal(raw, &got); err != nil {
					t.Fatalf("definition_of_done output not object JSON: %v", err)
				}
				for _, want := range []string{"narrative", "goals", "current_state", "module_shape"} {
					if _, ok := got[want]; !ok {
						t.Fatalf("definition_of_done missing %q: %v", want, got)
					}
				}
			},
		},
		{
			name: "goals",
			args: []string{"template", "--json", "--section", "definition_of_done", "--subsection", "goals"},
			check: func(t *testing.T, raw []byte) {
				t.Helper()
				var got []any
				if err := json.Unmarshal(raw, &got); err != nil {
					t.Fatalf("goals output not array JSON: %v", err)
				}
				if len(got) != 1 {
					t.Fatalf("goals length = %d, want 1", len(got))
				}
			},
		},
		{
			name: "implementation",
			args: []string{"template", "--json", "--section", "implementation"},
			check: func(t *testing.T, raw []byte) {
				t.Helper()
				var got []any
				if err := json.Unmarshal(raw, &got); err != nil {
					t.Fatalf("implementation output not array JSON: %v", err)
				}
				if len(got) != 1 {
					t.Fatalf("implementation length = %d, want 1", len(got))
				}
			},
		},
		{
			name: "implementation_step",
			args: []string{"template", "--json", "--section", "implementation", "--subsection", "1"},
			check: func(t *testing.T, raw []byte) {
				t.Helper()
				var got map[string]any
				if err := json.Unmarshal(raw, &got); err != nil {
					t.Fatalf("implementation step output not object JSON: %v", err)
				}
				for _, want := range []string{"title", "summary", "file_changes"} {
					if _, ok := got[want]; !ok {
						t.Fatalf("implementation step missing %q: %v", want, got)
					}
				}
			},
		},
		{
			name: "verification",
			args: []string{"template", "--json", "--section", "verification"},
			check: func(t *testing.T, raw []byte) {
				t.Helper()
				var got map[string]any
				if err := json.Unmarshal(raw, &got); err != nil {
					t.Fatalf("verification output not object JSON: %v", err)
				}
				for _, want := range []string{"summary", "automated", "manual"} {
					if _, ok := got[want]; !ok {
						t.Fatalf("verification missing %q: %v", want, got)
					}
				}
			},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if exitCode := Execute(tc.args, &stdout, &stderr); exitCode != 0 {
				t.Fatalf("Execute(%v) exit code = %d, want 0, stderr = %q", tc.args, exitCode, stderr.String())
			}
			tc.check(t, stdout.Bytes())
		})
	}
}

func TestTemplateUsageErrors(t *testing.T) {
	tests := []struct {
		name    string
		args    []string
		wantErr string
	}{
		{
			name:    "section_requires_json",
			args:    []string{"template", "--section", "overview"},
			wantErr: "--section requires --json",
		},
		{
			name:    "overview_rejects_subsection",
			args:    []string{"template", "--json", "--section", "overview", "--subsection", "1"},
			wantErr: "does not support subsections",
		},
		{
			name:    "subsection_requires_section",
			args:    []string{"template", "--json", "--subsection", "1"},
			wantErr: "--subsection requires --section",
		},
		{
			name:    "md_and_json_are_mutually_exclusive",
			args:    []string{"template", "--md", "--json"},
			wantErr: "--md and --json are mutually exclusive",
		},
		{
			name:    "implementation_subsection_must_be_numeric",
			args:    []string{"template", "--json", "--section", "implementation", "--subsection", "banana"},
			wantErr: "1-based integer index",
		},
		{
			name:    "implementation_subsection_must_be_in_range",
			args:    []string{"template", "--json", "--section", "implementation", "--subsection", "2"},
			wantErr: "out of range",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if exitCode := Execute(tc.args, &stdout, &stderr); exitCode != 2 {
				t.Fatalf("Execute(%v) exit code = %d, want 2; stderr = %q", tc.args, exitCode, stderr.String())
			}
			if !strings.Contains(stderr.String(), tc.wantErr) {
				t.Fatalf("stderr %q does not contain %q", stderr.String(), tc.wantErr)
			}
		})
	}
}

func TestTemplateHelpPrintsWorkflow(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"template", "--help"}, &stdout, &stderr); exitCode != 0 {
		t.Fatalf("Execute(template --help) exit code = %d, want 0, stderr = %q", exitCode, stderr.String())
	}

	// Anchor: PLACEHOLDER is the irreducible thing AIs need to learn from
	// this help text. Wording around it is free to drift.
	if !strings.Contains(stdout.String(), "PLACEHOLDER") {
		t.Fatalf("template --help missing PLACEHOLDER anchor: %q", stdout.String())
	}
}

func TestShowSchemaRemoved(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"show-schema"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("Execute(show-schema) exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "unknown command: show-schema") {
		t.Fatalf("show-schema stderr missing unknown-command message: %q", stderr.String())
	}
}

func TestGenerateRemoved(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"generate"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("Execute(generate) exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "unknown command: generate") {
		t.Fatalf("generate stderr missing unknown-command message: %q", stderr.String())
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

func TestDryRunDiffExitsOneOnChange(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	if err := os.WriteFile(out, []byte("stale\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin", "--diff", "--dry-run"}, &stdout, &stderr); exit != 1 {
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

func TestDiffWritesByDefault(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin", "--diff"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
	})
	if _, err := os.Stat(out); err != nil {
		t.Fatalf("expected output: %v", err)
	}
}

func TestDefaultWritePrintsOutputPath(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
		if !strings.Contains(stdout.String(), out) {
			t.Fatalf("stdout missing output path; got %q", stdout.String())
		}
	})
}

func TestJSONErrorsFlagEmitsStructuredJSON(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"validate", "--json-errors", "/no/such/path"}, &stdout, &stderr); exit != 1 {
		t.Fatalf("exit %d want 1", exit)
	}
	if strings.Contains(stderr.String(), "planner: reading JSON from stdin") || strings.Contains(stderr.String(), "repaired JSON input") {
		t.Fatalf("unexpected informational stderr in json mode: %q", stderr.String())
	}
	var got struct {
		Code         string `json:"code"`
		Message      string `json:"message"`
		RecoveryHint string `json:"recovery_hint"`
	}
	if err := json.Unmarshal(bytes.TrimSpace(stderr.Bytes()), &got); err != nil {
		t.Fatalf("stderr is not JSON: %v; raw=%q", err, stderr.String())
	}
	if got.Code != "READ_INPUT" {
		t.Fatalf("code=%q want READ_INPUT", got.Code)
	}
	if got.Message == "" {
		t.Fatal("empty message")
	}
}

func TestCreateRejectsWriteFlag(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		exit := Execute([]string{"create", out, "--stdin", "--write"}, &stdout, &stderr)
		if exit != 2 {
			t.Fatalf("exit %d want 2; stderr %q", exit, stderr.String())
		}
		if !strings.Contains(stderr.String(), "unknown flag \"--write\"") {
			t.Fatalf("expected rejected --write flag, got %q", stderr.String())
		}
	})
}

// withStdin routes data through os.Stdin for the duration of fn via a real
// os.Pipe (no mock). Tests exercise the production Execute path end-to-end.
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
	exit := Execute([]string{"replace", planPath, patchPath, outputPath, "--section", "overview"}, &stdout, &stderr)
	if exit == 0 {
		t.Fatalf("expected non-zero exit on write failure, got 0")
	}
	if strings.Contains(stdout.String(), "{") {
		t.Fatalf("replace result JSON must not be emitted on write failure; stdout = %q", stdout.String())
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

func TestRepairFixesUnescapedNewlineInDiffField(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	withStdin(t, brokenDiffJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
		if !strings.Contains(stderr.String(), "repaired") {
			t.Fatalf("expected repair notice on stderr, got %q", stderr.String())
		}
	})
	rendered, err := os.ReadFile(out)
	if err != nil {
		t.Fatalf("read rendered output: %v", err)
	}
	for _, want := range []string{"@@ -1 +1 @@", "-a", "+b"} {
		if !strings.Contains(string(rendered), want) {
			t.Fatalf("rendered output missing %q:\n%s", want, string(rendered))
		}
	}
}

func TestRepairNoticeIsSuppressedInJSONMode(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	withStdin(t, brokenDiffJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin", "--json-errors"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
		if stderr.Len() != 0 {
			t.Fatalf("expected quiet stderr in json mode, got %q", stderr.String())
		}
	})
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

func TestRepairIsNoopOnValidJSON(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/out.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
		if strings.Contains(stderr.String(), "repaired") {
			t.Fatalf("unexpected repair notice for valid JSON: %q", stderr.String())
		}
	})
}

func TestRepairDoesNotClaimSuccessWhenRepairFails(t *testing.T) {
	input := []byte("{bad}")
	withStdin(t, input, func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"validate", "--stdin"}, &stdout, &stderr); exit != 1 {
			t.Fatalf("exit %d want 1, stderr %q", exit, stderr.String())
		}
		if strings.Contains(stderr.String(), "repaired JSON input") {
			t.Fatalf("unexpected repair notice: %q", stderr.String())
		}
		if !strings.Contains(stderr.String(), "decode plan JSON") {
			t.Fatalf("expected decode failure, got %q", stderr.String())
		}
	})
}

// brokenDiffJSON takes validPlanJSON and replaces the \n escape sequences in
// the diff field value with literal newline bytes, producing invalid JSON that
// mirrors the primary LLM output failure mode.
func brokenDiffJSON() []byte {
	return []byte(strings.ReplaceAll(string(validPlanJSON()), `\n`, "\n"))
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
		exit := Execute([]string{"replace", src, src, "--section", "overview", "--stdin"}, &stdout, &stderr)
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

func TestReadPlanFromReturnsTypedDecodeError(t *testing.T) {
	dir := t.TempDir()
	path := dir + "/bad.json"
	if err := os.WriteFile(path, []byte(`{"title":123}`), 0o644); err != nil {
		t.Fatal(err)
	}
	var stderr bytes.Buffer
	_, err := readPlanFrom([]string{path}, false, &stderr)
	if err == nil {
		t.Fatal("expected typed decode error")
	}
	var cliErr *PlannerCLIError
	if !errors.As(err, &cliErr) {
		t.Fatalf("expected PlannerCLIError, got %T", err)
	}
	if cliErr.Code != PlannerDecodeInputError {
		t.Fatalf("got code %v, want %v", cliErr.Code, PlannerDecodeInputError)
	}
}

func TestDiffWritesButDoesNotEmitResult(t *testing.T) {
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
		exit := Execute([]string{"replace", src, src, "--section", "overview", "--stdin", "--diff"}, &stdout, &stderr)
		if exit != 0 {
			t.Fatalf("exit %d stderr %q", exit, stderr.String())
		}
		if strings.Contains(stdout.String(), `"section"`) {
			t.Fatalf("replace result JSON must not appear on stdout when --diff is set; stdout = %q", stdout.String())
		}
	})
}

func TestReplaceInvalidSourceMarkdownReturnsDecodePlanMarkdownError(t *testing.T) {
	dir := t.TempDir()
	src := dir + "/not-a-plan.md"
	if err := os.WriteFile(src, []byte("# not a planner doc\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	patch := []byte(`"Fresh overview text."`)
	withStdin(t, patch, func() {
		var stdout, stderr bytes.Buffer
		exit := Execute([]string{"replace", src, src, "--section", "overview", "--stdin"}, &stdout, &stderr)
		if exit != 1 {
			t.Fatalf("exit %d want 1 stderr %q", exit, stderr.String())
		}
		if strings.Contains(stderr.String(), "decode replace patch") {
			t.Fatalf("misclassified replace error: %q", stderr.String())
		}
		if !strings.Contains(stderr.String(), "decode plan markdown") {
			t.Fatalf("expected plan markdown decode error, got %q", stderr.String())
		}
	})
}

func TestCreateBaselineReadFailureExitsOne(t *testing.T) {
	dir := t.TempDir()
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		exit := Execute([]string{"create", dir, "--stdin", "--diff"}, &stdout, &stderr)
		if exit != 1 {
			t.Fatalf("exit %d want 1 stderr %q", exit, stderr.String())
		}
	})
}

// firstStderrJSON unmarshals the first non-empty stderr line as the planner
// error envelope. Tests use it to assert the --json-errors contract: every
// failure path emits one parseable JSON object with a stable code.
func firstStderrJSON(t *testing.T, stderr *bytes.Buffer) (code, message string) {
	t.Helper()
	line := bytes.TrimSpace(stderr.Bytes())
	if len(line) == 0 {
		t.Fatal("stderr is empty")
	}
	if nl := bytes.IndexByte(line, '\n'); nl >= 0 {
		line = line[:nl]
	}
	var got struct {
		Code    string `json:"code"`
		Message string `json:"message"`
	}
	if err := json.Unmarshal(line, &got); err != nil {
		t.Fatalf("first stderr line is not JSON: %v; raw=%q", err, stderr.String())
	}
	return got.Code, got.Message
}

func TestJSONErrorsCoversUnknownCommand(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"--json-errors", "bogus"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit %d want 2; stderr %q", exit, stderr.String())
	}
	code, _ := firstStderrJSON(t, &stderr)
	if code != "USAGE" {
		t.Fatalf("code=%q want USAGE", code)
	}
}

func TestJSONErrorsCoversUsageFailure(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"create", "--json-errors"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit %d want 2; stderr %q", exit, stderr.String())
	}
	code, msg := firstStderrJSON(t, &stderr)
	if code != "USAGE" {
		t.Fatalf("code=%q want USAGE", code)
	}
	if !strings.Contains(msg, "usage: planner create") {
		t.Fatalf("message %q missing usage banner", msg)
	}
}

func TestJSONErrorsCoversRuntimeFailure(t *testing.T) {
	// Pass a directory as outputPath so runPreview's baseline read fails.
	// The error must be coded READ_INPUT, not the RUNTIME or VALIDATE_INPUT
	// fallback, proving runPreview constructs a typed error at the call site.
	dir := t.TempDir()
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		exit := Execute([]string{"create", dir, "--stdin", "--diff", "--json-errors"}, &stdout, &stderr)
		if exit != 1 {
			t.Fatalf("exit %d want 1; stderr %q", exit, stderr.String())
		}
		code, _ := firstStderrJSON(t, &stderr)
		if code != "READ_INPUT" {
			t.Fatalf("code=%q want READ_INPUT", code)
		}
	})
}
