package internal

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"os"
	"strings"
	"testing"
)

func TestHelpTextIncludesRules(t *testing.T) {
	help := buildHelpText()

	// Positive anchors: every command we ship must appear in help so AIs can
	// discover the current surface from `planner help` alone.
	for _, command := range []string{
		"planner template",
		"planner check",
		"planner create",
		"planner inspect",
	} {
		if !strings.Contains(help, command) {
			t.Fatalf("buildHelpText() missing command %q", command)
		}
	}
	if strings.Contains(help, "planner validate") {
		t.Fatal("buildHelpText() must not mention planner validate")
	}

	// Negative anchors: deleted commands and removed flags must not reappear.
	for _, banned := range []string{"show-schema", "planner generate", "planner replace", "planner patch", "--write"} {
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

	plan, err := DecodePlan(stdout.Bytes())
	if err != nil {
		t.Fatalf("template JSON output is not valid plan JSON: %v", err)
	}
	if err := ValidatePlan(plan); err != nil {
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
			name:    "raw_and_json_are_mutually_exclusive",
			args:    []string{"template", "--raw", "--json", "--section", "title"},
			wantErr: "--raw is mutually exclusive with --md and --json",
		},
		{
			name:    "raw_requires_section",
			args:    []string{"template", "--raw"},
			wantErr: "--raw requires --section",
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
		{
			name:    "verification_bad_subsection",
			args:    []string{"template", "--json", "--section", "verification", "--subsection", "bogus"},
			wantErr: "invalid verification subsection",
		},
		{
			name:    "md_does_not_accept_selectors",
			args:    []string{"template", "--md", "--section", "implementation", "--subsection", "1", "--field", "diff"},
			wantErr: "--md does not accept selectors",
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

func TestTemplateAcceptsFullFieldGrammar(t *testing.T) {
	tests := []struct {
		name       string
		args       []string
		wantSubstr string
	}{
		{
			name:       "title",
			args:       []string{"template", "--json", "--section", "title"},
			wantSubstr: fmt.Sprintf("max %d chars", MaxTitleLength),
		},
		{
			name:       "verification summary",
			args:       []string{"template", "--json", "--section", "verification", "--subsection", "summary"},
			wantSubstr: "\"<optional summary>\"",
		},
		{
			name:       "verification automated",
			args:       []string{"template", "--json", "--section", "verification", "--subsection", "automated"},
			wantSubstr: "[",
		},
		{
			name:       "verification manual",
			args:       []string{"template", "--json", "--section", "verification", "--subsection", "manual"},
			wantSubstr: "[",
		},
		{
			name:       "step title field",
			args:       []string{"template", "--json", "--section", "implementation", "--subsection", "1", "--field", "title"},
			wantSubstr: fmt.Sprintf("max %d chars", MaxTitleLength),
		},
		{
			name:       "step summary field",
			args:       []string{"template", "--json", "--section", "implementation", "--subsection", "1", "--field", "summary"},
			wantSubstr: fmt.Sprintf("max %d chars", MaxStepSummaryLength),
		},
		{
			name:       "filename field",
			args:       []string{"template", "--json", "--section", "implementation", "--subsection", "1", "--file", "f", "--field", "filename"},
			wantSubstr: "\"<filename>\"",
		},
		{
			name:       "explanation field",
			args:       []string{"template", "--json", "--section", "implementation", "--subsection", "1", "--file", "f", "--field", "explanation"},
			wantSubstr: "\"<explanation>\"",
		},
		{
			name:       "diff field raw",
			args:       []string{"template", "--section", "implementation", "--subsection", "1", "--file", "f", "--field", "diff"},
			wantSubstr: "--- a/<path>",
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if exitCode := Execute(tc.args, &stdout, &stderr); exitCode != 0 {
				t.Fatalf("Execute(%v) exit code = %d, stderr = %q", tc.args, exitCode, stderr.String())
			}
			if !strings.Contains(stdout.String(), tc.wantSubstr) {
				t.Fatalf("stdout %q does not contain %q", stdout.String(), tc.wantSubstr)
			}
		})
	}
}

func TestTemplateRejectsInvalidGrammar(t *testing.T) {
	tests := []struct {
		name    string
		args    []string
		wantErr string
	}{
		{
			name:    "title_with_subsection",
			args:    []string{"template", "--json", "--section", "title", "--subsection", "1"},
			wantErr: "--section title accepts no other selectors",
		},
		{
			name:    "title_field_with_file",
			args:    []string{"template", "--json", "--section", "implementation", "--subsection", "1", "--file", "f", "--field", "title"},
			wantErr: "--field title does not take --file",
		},
		{
			name:    "filename_without_file",
			args:    []string{"template", "--json", "--section", "implementation", "--subsection", "1", "--field", "filename"},
			wantErr: "--field filename requires --file F",
		},
		{
			name:    "file_without_field",
			args:    []string{"template", "--json", "--section", "implementation", "--subsection", "1", "--file", "f"},
			wantErr: "--file requires --field",
		},
	}
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if exitCode := Execute(tc.args, &stdout, &stderr); exitCode != 2 {
				t.Fatalf("Execute(%v) exit code = %d, stderr = %q", tc.args, exitCode, stderr.String())
			}
			if !strings.Contains(stderr.String(), tc.wantErr) {
				t.Fatalf("stderr %q does not contain %q", stderr.String(), tc.wantErr)
			}
		})
	}
}

func TestTemplateRawScalar(t *testing.T) {
	plan := BuildPlanTemplate()
	decode := func(section, subsection, file, field string) string {
		t.Helper()
		raw, err := MarshalSection(plan, section, subsection, file, field)
		if err != nil {
			t.Fatalf("MarshalSection(%s,%s,%s,%s): %v", section, subsection, file, field, err)
		}
		var got string
		if err := json.Unmarshal(raw, &got); err != nil {
			t.Fatalf("json.Unmarshal scalar: %v", err)
		}
		return got
	}
	cases := []struct {
		name       string
		args       []string
		section    string
		subsection string
		file       string
		field      string
	}{
		{name: "title", args: []string{"template", "--raw", "--section", "title"}, section: "title"},
		{name: "overview", args: []string{"template", "--raw", "--section", "overview"}, section: "overview"},
		{name: "narrative", args: []string{"template", "--raw", "--section", "definition_of_done", "--subsection", "narrative"}, section: "definition_of_done", subsection: "narrative"},
		{name: "current_state", args: []string{"template", "--raw", "--section", "definition_of_done", "--subsection", "current_state"}, section: "definition_of_done", subsection: "current_state"},
		{name: "module_shape", args: []string{"template", "--raw", "--section", "definition_of_done", "--subsection", "module_shape"}, section: "definition_of_done", subsection: "module_shape"},
		{name: "step_title", args: []string{"template", "--raw", "--section", "implementation", "--subsection", "1", "--field", "title"}, section: "implementation", subsection: "1", field: "title"},
		{name: "step_summary", args: []string{"template", "--raw", "--section", "implementation", "--subsection", "1", "--field", "summary"}, section: "implementation", subsection: "1", field: "summary"},
		{name: "filename", args: []string{"template", "--raw", "--section", "implementation", "--subsection", "1", "--file", "f", "--field", "filename"}, section: "implementation", subsection: "1", file: "f", field: "filename"},
		{name: "explanation", args: []string{"template", "--raw", "--section", "implementation", "--subsection", "1", "--file", "f", "--field", "explanation"}, section: "implementation", subsection: "1", file: "f", field: "explanation"},
		{name: "verification_summary", args: []string{"template", "--raw", "--section", "verification", "--subsection", "summary"}, section: "verification", subsection: "summary"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if exitCode := Execute(tc.args, &stdout, &stderr); exitCode != 0 {
				t.Fatalf("Execute(%v) exit code = %d, stderr = %q", tc.args, exitCode, stderr.String())
			}
			expected := decode(tc.section, tc.subsection, tc.file, tc.field)
			if got := stdout.String(); got != expected+"\n" {
				t.Fatalf("stdout %q does not equal raw scalar %q", got, expected+"\n")
			}
		})
	}
}

func TestTemplateRawRejectsStructured(t *testing.T) {
	cases := [][]string{
		{"template", "--json-errors", "--raw", "--section", "definition_of_done", "--subsection", "goals"},
		{"template", "--json-errors", "--raw", "--section", "verification", "--subsection", "automated"},
		{"template", "--json-errors", "--raw", "--section", "verification", "--subsection", "manual"},
		{"template", "--json-errors", "--raw", "--section", "definition_of_done"},
		{"template", "--json-errors", "--raw", "--section", "implementation", "--subsection", "1"},
	}
	for _, args := range cases {
		var stdout bytes.Buffer
		var stderr bytes.Buffer
		if exit := Execute(args, &stdout, &stderr); exit != 2 {
			t.Fatalf("args %v: exit %d want 2", args, exit)
		}
		code, msg := firstStderrJSON(t, &stderr)
		if code != "USAGE" {
			t.Fatalf("args %v: code=%q want USAGE", args, code)
		}
		if !strings.Contains(msg, "--raw") {
			t.Fatalf("args %v: message %q does not mention --raw", args, msg)
		}
	}
}

func TestReadRawScalarStripsTrailingNewline(t *testing.T) {
	t.Run("stdin_lf", func(t *testing.T) {
		withStdin(t, []byte("raw text\n"), func() {
			got, err := readRawScalar("", true)
			if err != nil {
				t.Fatalf("readRawScalar(stdin): %v", err)
			}
			if string(got) != "raw text" {
				t.Fatalf("readRawScalar(stdin) = %q, want %q", got, "raw text")
			}
		})
	})
	t.Run("file_crlf", func(t *testing.T) {
		dir := t.TempDir()
		path := dir + "/raw.txt"
		if err := os.WriteFile(path, []byte("raw text\r\n"), 0o644); err != nil {
			t.Fatal(err)
		}
		got, err := readRawScalar(path, false)
		if err != nil {
			t.Fatalf("readRawScalar(file): %v", err)
		}
		if string(got) != "raw text" {
			t.Fatalf("readRawScalar(file) = %q, want %q", got, "raw text")
		}
	})
	t.Run("file_double_lf", func(t *testing.T) {
		dir := t.TempDir()
		path := dir + "/raw.txt"
		if err := os.WriteFile(path, []byte("raw text\n\n"), 0o644); err != nil {
			t.Fatal(err)
		}
		got, err := readRawScalar(path, false)
		if err != nil {
			t.Fatalf("readRawScalar(file): %v", err)
		}
		if string(got) != "raw text\n" {
			t.Fatalf("readRawScalar(file) = %q, want %q", got, "raw text\n")
		}
	})
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

func TestTemplateHelpListsFieldGrammar(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"template", "--help"}, &stdout, &stderr); exitCode != 0 {
		t.Fatalf("Execute(template --help) exit code = %d, stderr = %q", exitCode, stderr.String())
	}
	for _, want := range []string{
		"--field diff",
		"--file <filename>",
		"--raw",
		"raw text for scalar string selectors",
		"Section-level JSON shape: title",
	} {
		if !strings.Contains(stdout.String(), want) {
			t.Fatalf("template --help missing %q: %q", want, stdout.String())
		}
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
	if exit := Execute([]string{"check", "--json-errors", "/no/such/path.json"}, &stdout, &stderr); exit != 1 {
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

func TestRunCheckMarkdown(t *testing.T) {
	plan, err := DecodePlan(validPlanJSON())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	dir := t.TempDir()
	path := dir + "/plan.md"
	if err := os.WriteFile(path, []byte(rendered), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", path}, &stdout, &stderr); exit != 0 {
		t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stdout.String(), "OK") {
		t.Fatalf("expected OK in stdout, got %q", stdout.String())
	}
}

func TestRunCheckJSON(t *testing.T) {
	dir := t.TempDir()
	path := dir + "/plan.json"
	if err := os.WriteFile(path, validPlanJSON(), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", path}, &stdout, &stderr); exit != 0 {
		t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stdout.String(), "OK") {
		t.Fatalf("expected OK in stdout, got %q", stdout.String())
	}
}

func TestRunCheckStdinFormatMd(t *testing.T) {
	plan, err := DecodePlan(validPlanJSON())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	withStdin(t, []byte(rendered), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"check", "--format", "md", "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
		}
		if !strings.Contains(stdout.String(), "OK") {
			t.Fatalf("expected OK in stdout, got %q", stdout.String())
		}
	})
}

func TestRunCheckStdinFormatJson(t *testing.T) {
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"check", "--format", "json", "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
		}
		if !strings.Contains(stdout.String(), "OK") {
			t.Fatalf("expected OK in stdout, got %q", stdout.String())
		}
	})
}

func TestRunCheckStdinRequiresFormat(t *testing.T) {
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"check", "--stdin"}, &stdout, &stderr); exit != 2 {
			t.Fatalf("exit=%d want 2; stderr=%q", exit, stderr.String())
		}
		if !strings.Contains(stderr.String(), "--format") {
			t.Fatalf("expected --format mention, got %q", stderr.String())
		}
	})
}

func TestRunCheckUnknownExtensionRequiresFormat(t *testing.T) {
	dir := t.TempDir()
	path := dir + "/plan.txt"
	if err := os.WriteFile(path, []byte("{}"), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", path}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d want 2; stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stderr.String(), "--format") {
		t.Fatalf("expected --format mention, got %q", stderr.String())
	}
}

func TestRunCheckUnknownFormat(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", "--format", "xml", "/dev/null"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d want 2; stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stderr.String(), "not valid; use md or json") {
		t.Fatalf("expected invalid format error, got %q", stderr.String())
	}
}

func TestRunCheckAggregatesViolations(t *testing.T) {
	plan := Plan{
		Title:    "",
		Overview: "",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    strings.Repeat("n", 501),
			Goals:        []ChecklistItem{{Text: "goal"}},
			CurrentState: "current state",
			ModuleShape:  "planner/check",
		},
		Implementation: []Step{
			{
				Title:   "step title",
				Summary: "step summary",
				FileChanges: []FileChange{{
					Filename:    "planner/check/check.go",
					Explanation: "explanation",
					Diff:        "@@ -1 +1 @@\n- old\n+ new",
				}},
			},
		},
		Verification: &Verification{
			Automated: []ChecklistItem{{Text: "automation"}},
			Manual:    []ChecklistItem{{Text: "manual"}},
		},
	}
	raw, err := json.Marshal(plan)
	if err != nil {
		t.Fatalf("Marshal: %v", err)
	}

	dir := t.TempDir()
	path := dir + "/plan.json"
	if err := os.WriteFile(path, raw, 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", path}, &stdout, &stderr); exit != 1 {
		t.Fatalf("Execute(check) exit = %d, want 1; stderr = %q", exit, stderr.String())
	}
	for _, want := range []string{
		"title is required",
		"overview is required",
		fmt.Sprintf("definition_of_done.narrative must be no more than %d characters", MaxDoDNarrativeLength),
	} {
		if !strings.Contains(stderr.String(), want) {
			t.Fatalf("stderr missing %q:\n%s", want, stderr.String())
		}
	}
}

func TestValidateCommandRemoved(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"validate"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d want 2; stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stderr.String(), "unknown command: validate") {
		t.Fatalf("unexpected stderr: %q", stderr.String())
	}
}

func TestPatchCommandRemoved(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"patch", "--help"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d want 2; stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
	}
	if !strings.Contains(stderr.String(), "unknown command: patch") {
		t.Fatalf("unexpected stderr: %q", stderr.String())
	}
}

func TestBehavioralEditsCoverApprovedGrammar(t *testing.T) {
	dir := t.TempDir()
	planPath := writeBehavioralPlan(t, dir)
	out := dir + "/out.md"

	runPlannerOK(t, []string{"title", "set", planPath, out, "New title"}, nil)
	assertParsed(t, out, func(p Plan) {
		if p.Title != "New title" {
			t.Fatalf("title=%q", p.Title)
		}
	})

	runPlannerOK(t, []string{"dod", "goal", "set", out, out, "--goal", "1", "renamed goal"}, nil)
	assertParsed(t, out, func(p Plan) {
		if p.DefinitionOfDone.Goals[0].Text != "renamed goal" || p.DefinitionOfDone.Goals[0].Status != StatusDone {
			t.Fatalf("goal not updated with status preserved: %#v", p.DefinitionOfDone.Goals[0])
		}
	})

	runPlannerOK(t, []string{"implementation", "step", "file-change", "add", out, out, "--step", "1", "--filename", "f", "--explanation", "second", "--diff-stdin"}, []byte("@@ -1 +1 @@\n-x\n+y"))
	runPlannerOK(t, []string{"implementation", "step", "file-change", "filename", "set", out, out, "--step", "1", "--change", "2", "renamed"}, nil)
	runPlannerOK(t, []string{"implementation", "step", "file-change", "diff", "set", out, out, "--step", "1", "--change", "2", "--stdin"}, []byte("raw diff bytes"))
	assertParsed(t, out, func(p Plan) {
		if got := p.Implementation[0].FileChanges[1].Filename; got != "renamed" {
			t.Fatalf("second filename=%q", got)
		}
		if got := p.Implementation[0].FileChanges[1].Diff; got != "raw diff bytes" {
			t.Fatalf("second diff=%q", got)
		}
	})

	runPlannerOK(t, []string{"verification", "automated", "set", out, out, "--item", "1", "new automated"}, nil)
	assertParsed(t, out, func(p Plan) {
		if p.Verification.Automated[0].Text != "new automated" || p.Verification.Automated[0].Status != StatusDone {
			t.Fatalf("automated not updated with status preserved: %#v", p.Verification.Automated[0])
		}
	})
}

func TestBehavioralRemovalAndUsageFailures(t *testing.T) {
	dir := t.TempDir()
	planPath := writeBehavioralPlan(t, dir)
	out := dir + "/out.md"

	for _, tc := range []struct {
		name string
		args []string
		want string
	}{
		{"goal", []string{"dod", "goal", "remove", planPath, out, "--goal", "1"}, "cannot remove the final definition_of_done goal"},
		{"step", []string{"implementation", "step", "remove", planPath, out, "--step", "1"}, "cannot remove the final implementation step"},
		{"change", []string{"implementation", "step", "file-change", "remove", planPath, out, "--step", "1", "--change", "1"}, "cannot remove the final file change from a step"},
		{"json", []string{"--json-errors", "title", "set", planPath, out, "   "}, "USAGE"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			if exit := Execute(tc.args, &stdout, &stderr); exit != 2 {
				t.Fatalf("exit=%d want 2 stderr=%q", exit, stderr.String())
			}
			if !strings.Contains(stderr.String(), tc.want) {
				t.Fatalf("stderr missing %q: %q", tc.want, stderr.String())
			}
		})
	}
}

func TestBehavioralEditRejectsUnknownValueFlag(t *testing.T) {
	dir := t.TempDir()
	planPath := writeBehavioralPlan(t, dir)
	out := dir + "/out.md"

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"title", "set", planPath, out, "New title", "--typo", "x"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d want 2 stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stderr.String(), "unknown flag --typo") {
		t.Fatalf("stderr missing unknown flag: %q", stderr.String())
	}
	if _, err := os.Stat(out); !os.IsNotExist(err) {
		t.Fatalf("output should not be written, stat err = %v", err)
	}
}

func TestStructuredBehavioralEditMalformedMarkdownJSONError(t *testing.T) {
	dir := t.TempDir()
	bad := dir + "/bad.md"
	out := dir + "/out.md"
	if err := os.WriteFile(bad, []byte("not a canonical plan"), 0o644); err != nil {
		t.Fatal(err)
	}

	var stdout, stderr bytes.Buffer
	exit := Execute([]string{"--json-errors", "dod", "goal", "set", bad, out, "--goal", "1", "renamed"}, &stdout, &stderr)
	if exit != 1 {
		t.Fatalf("exit=%d want 1 stderr=%q", exit, stderr.String())
	}
	code, _ := firstStderrJSON(t, &stderr)
	if code != "DECODE_INPUT" {
		t.Fatalf("code=%q want DECODE_INPUT", code)
	}
	if _, err := os.Stat(out); !os.IsNotExist(err) {
		t.Fatalf("output should not be written, stat err = %v", err)
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

func writeBehavioralPlan(t *testing.T, dir string) string {
	t.Helper()
	path := dir + "/plan.md"
	withStdin(t, mustJSON(Plan{
		Title:    "T",
		Overview: "O",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "N",
			Goals:        []ChecklistItem{{Text: "g", Status: StatusDone}},
			CurrentState: "C",
			ModuleShape:  "M",
		},
		Implementation: []Step{{
			Title:   "T",
			Summary: "S",
			FileChanges: []FileChange{{
				Filename:    "f",
				Explanation: "e",
				Diff:        "@@ -1 +1 @@\n-a\n+b",
			}},
		}},
		Verification: &Verification{
			Summary:   "",
			Automated: []ChecklistItem{{Text: "A", Status: StatusDone}},
			Manual:    []ChecklistItem{{Text: "M"}},
		},
	}), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", path, "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("create exit=%d stderr=%q", exit, stderr.String())
		}
	})
	return path
}

func runPlannerOK(t *testing.T, args []string, stdin []byte) {
	t.Helper()
	run := func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute(args, &stdout, &stderr); exit != 0 {
			t.Fatalf("Execute(%v) exit=%d stderr=%q stdout=%q", args, exit, stderr.String(), stdout.String())
		}
	}
	if stdin != nil {
		withStdin(t, stdin, run)
		return
	}
	run()
}

func assertParsed(t *testing.T, path string, check func(Plan)) {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	parsed, err := ParseMarkdown(string(raw))
	if err != nil {
		t.Fatalf("ParseMarkdown: %v\n%s", err, string(raw))
	}
	check(parsed.Plan)
}

func validPlanJSON() []byte {
	return mustJSON(Plan{
		Title:    "T",
		Overview: "O",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "N",
			Goals:        []ChecklistItem{{Text: "g"}},
			CurrentState: "C",
			ModuleShape:  "M",
		},
		Implementation: []Step{{
			Title:   "T",
			Summary: "S",
			FileChanges: []FileChange{{
				Filename:    "f",
				Explanation: "e",
				Diff:        "@@ -1 +1 @@\n-a\n+b",
			}},
		}},
		Verification: &Verification{
			Summary:   "",
			Automated: []ChecklistItem{{Text: "A"}},
			Manual:    []ChecklistItem{{Text: "M"}},
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

func TestCheckSurfacesSchemaErrors(t *testing.T) {
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
			assertCommandError(t, []string{"check", path}, "check:", tc.wantErr, 1)
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
		if exit := Execute([]string{"check", "--format", "json", "--stdin"}, &stdout, &stderr); exit != 1 {
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

func TestInspectOutputIsValidPlan(t *testing.T) {
	dir := t.TempDir()
	src := dir + "/plan.md"
	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", src, "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("seed exit %d stderr %q", exit, stderr.String())
		}
	})

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"inspect", src}, &stdout, &stderr); exit != 0 {
		t.Fatalf("inspect exit %d stderr %q", exit, stderr.String())
	}

	plan, err := DecodePlan(stdout.Bytes())
	if err != nil {
		t.Fatalf("inspect output not valid plan JSON: %v", err)
	}
	if err := ValidatePlan(plan); err != nil {
		t.Fatalf("inspect output does not validate: %v", err)
	}
	if plan.Title == "" || len(plan.Implementation) == 0 || plan.Verification == nil {
		t.Fatalf("inspect output missing plan content: %#v", plan)
	}
}

func TestInspectOutputOmitsFrontmatterFields(t *testing.T) {
	dir := t.TempDir()
	src := dir + "/plan.md"
	plan, err := DecodePlan(validPlanJSON())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	if err := os.WriteFile(src, []byte("---\ntags:\n  - \"#Ticket\"\ntype: issue\ntemplate_version: 1\ntopics: []\nstatus: open\nproject: PDEV-083\ndate_created: 2026-05-12\n---\n\n"+rendered), 0o644); err != nil {
		t.Fatal(err)
	}

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"inspect", src}, &stdout, &stderr); exit != 0 {
		t.Fatalf("inspect exit %d stderr %q", exit, stderr.String())
	}
	outPlan, err := DecodePlan(stdout.Bytes())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	if err := ValidatePlan(outPlan); err != nil {
		t.Fatalf("ValidatePlan: %v", err)
	}
	for _, want := range []string{"\"tags\"", "\"type\"", "\"template_version\"", "\"topics\"", "\"project\"", "\"date_created\""} {
		if strings.Contains(stdout.String(), want) {
			t.Fatalf("inspect output unexpectedly contains %q: %q", want, stdout.String())
		}
	}
}

func TestReplaceCommandRemoved(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"replace"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("Execute(replace) exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "unknown command: replace") {
		t.Fatalf("replace stderr missing unknown-command message: %q", stderr.String())
	}
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

func TestCreatePreservesExistingFrontmatterOnRewrite(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/plan.md"
	frontmatter := "---\ntags:\n  - \"#Ticket\"\ntype: issue\ntemplate_version: 1\ntopics: []\nstatus: open\nproject: PDEV-083\ndate_created: 2026-05-12\n---\n\n"
	if err := os.WriteFile(out, []byte(frontmatter+"old body\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	plan := Plan{
		Title:    "T",
		Overview: "O",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "N",
			Goals:        []ChecklistItem{{Text: "g"}},
			CurrentState: "C",
			ModuleShape:  "M",
		},
		Implementation: []Step{{
			Title:   "T",
			Summary: "S",
			FileChanges: []FileChange{{
				Filename:    "f",
				Explanation: "e",
				Diff:        "@@ -1 +1 @@\n-a\n+b",
			}},
		}},
		Verification: &Verification{
			Summary:   "",
			Automated: []ChecklistItem{{Text: "A"}},
			Manual:    []ChecklistItem{{Text: "M"}},
		},
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}

	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute([]string{"create", out, "--stdin"}, &stdout, &stderr); exit != 0 {
			t.Fatalf("create exit=%d stderr=%q", exit, stderr.String())
		}
	})

	raw, err := os.ReadFile(out)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	if got, want := string(raw), frontmatter+rendered; got != want {
		t.Fatalf("rewritten output mismatch\nwant:\n%s\n\ngot:\n%s", want, got)
	}
}

func TestJSONErrorsCoversUnsupportedFrontmatter(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/plan.md"
	if err := os.WriteFile(out, []byte("---\ntags:\n  - \"#ticket\"\ntype: issue\ntemplate_version: 1\ntopics: []\nstatus: open\nproject: PDEV-083\ndate_created: 2026-05-12\n---\n\nold body\n"), 0o644); err != nil {
		t.Fatal(err)
	}

	withStdin(t, validPlanJSON(), func() {
		var stdout, stderr bytes.Buffer
		exit := Execute([]string{"create", out, "--stdin", "--json-errors"}, &stdout, &stderr)
		if exit != 1 {
			t.Fatalf("exit %d want 1; stderr %q", exit, stderr.String())
		}
		code, _ := firstStderrJSON(t, &stderr)
		if code != "DECODE_INPUT" {
			t.Fatalf("code=%q want DECODE_INPUT", code)
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

func TestCreateDirOutputJSONError(t *testing.T) {
	// Directory output must fail as READ_INPUT.
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
