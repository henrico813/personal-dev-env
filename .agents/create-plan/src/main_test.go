package main

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"
)

func TestRun_NoArgs_PrintsHelp(t *testing.T) {
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)

	code := run(nil, stdout, stderr)

	if code != 0 {
		t.Fatalf("run() code = %d, want 0", code)
	}
	if !strings.Contains(stdout.String(), "show-schema") {
		t.Fatalf("stdout missing show-schema help: %q", stdout.String())
	}
	if stderr.Len() != 0 {
		t.Fatalf("stderr = %q, want empty", stderr.String())
	}
}

func TestRun_HelpAliases_PrintsSameHelp(t *testing.T) {
	for _, args := range [][]string{{"help"}, {"--help"}, {"-h"}} {
		stdout := new(bytes.Buffer)
		stderr := new(bytes.Buffer)
		code := run(args, stdout, stderr)
		if code != 0 {
			t.Fatalf("run(%v) code = %d, want 0", args, code)
		}
		if stdout.String() != helpText {
			t.Fatalf("run(%v) stdout = %q, want helpText", args, stdout.String())
		}
		if stderr.Len() != 0 {
			t.Fatalf("run(%v) stderr = %q, want empty", args, stderr.String())
		}
	}
}

func TestRun_UnknownCommand_PrintsHelpToStderr(t *testing.T) {
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	code := run([]string{"wat"}, stdout, stderr)
	if code != 2 {
		t.Fatalf("run() code = %d, want 2", code)
	}
	if stdout.Len() != 0 {
		t.Fatalf("stdout = %q, want empty", stdout.String())
	}
	if !strings.Contains(stderr.String(), "unknown command: wat") {
		t.Fatalf("stderr missing unknown command: %q", stderr.String())
	}
	if !strings.Contains(stderr.String(), "planner show-schema") {
		t.Fatalf("stderr missing help text: %q", stderr.String())
	}
}

func TestRunShowSchema_MatchesValidatorContract(t *testing.T) {
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)

	code := run([]string{"show-schema"}, stdout, stderr)

	if code != 0 {
		t.Fatalf("run() code = %d, want 0", code)
	}
	if stderr.Len() != 0 {
		t.Fatalf("stderr = %q, want empty", stderr.String())
	}

	var schema map[string]any
	if err := json.Unmarshal(stdout.Bytes(), &schema); err != nil {
		t.Fatalf("unmarshal schema: %v", err)
	}

	required := schema["required"].([]any)
	wantRequired := []string{"title", "overview", "definition_of_done", "implementation", "verification"}
	for i, want := range wantRequired {
		if required[i] != want {
			t.Fatalf("required[%d] = %v, want %q", i, required[i], want)
		}
	}

	properties := schema["properties"].(map[string]any)
	if _, ok := properties["verification"]; !ok {
		t.Fatal("schema missing verification property")
	}

	contract := schema["contract"].(map[string]any)
	guarantees := contract["guarantees"].([]any)
	if !containsAny(guarantees, "current validator requires the top-level verification field to be present") {
		t.Fatalf("guarantees missing verification rule: %v", guarantees)
	}
}

func TestDecodePlan_PreservesUnknownFieldBehavior(t *testing.T) {
	_, err := decodePlan([]byte(`{"title":"x","overview":"y","definition_of_done":{"narrative":"n","goals":["g"],"current_state":"c","module_shape":"m"},"implementation":[{"title":"s","summary":"sum","file_changes":[{"filename":"f","explanation":"e","language":"go","code":"func main() {}"}]}],"verification":{},"unknown":true}`))
	if err != nil {
		t.Fatalf("decodePlan() error = %v, want nil", err)
	}
}

func TestHelpText_CoversScratchAndRewriteFlows(t *testing.T) {
	for _, needle := range []string{
		"Scratch flow:",
		"Rewrite flow",
		"planner show-schema",
		"planner validate <plan.json>",
		"planner create <input.json> <output.md>",
	} {
		if !strings.Contains(helpText, needle) {
			t.Fatalf("helpText missing %q", needle)
		}
	}
}

func TestCreatePlan_RendersKnownGoodPlan(t *testing.T) {
	planPath := writeTempPlanJSON(t, mustJSON(t, validPlan()))
	outputPath := filepath.Join(t.TempDir(), "plan.md")

	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	code := run([]string{"create", planPath, outputPath}, stdout, stderr)

	if code != 0 {
		t.Fatalf("run() code = %d, want 0; stderr=%q", code, stderr.String())
	}
	if stderr.Len() != 0 {
		t.Fatalf("stderr = %q, want empty", stderr.String())
	}

	rendered := mustReadFile(t, outputPath)
	for _, section := range []string{
		"## Overview",
		"## Definition of Done",
		"## Implementation",
		"## Verification",
		"### 1. Render sample",
	} {
		if !strings.Contains(rendered, section) {
			t.Fatalf("rendered plan missing %q", section)
		}
	}
}

func TestValidate_PreservesUnknownFieldBehavior(t *testing.T) {
	body := mustJSON(t, validPlan())
	planPath := writeTempPlanJSON(t, strings.Replace(body, `"verification": {`, `"unknown": true, "verification": {`, 1))
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	code := run([]string{"validate", planPath}, stdout, stderr)
	if code != 0 {
		t.Fatalf("run() code = %d, want 0; stderr=%q", code, stderr.String())
	}
	if stdout.String() != "OK\n" {
		t.Fatalf("stdout = %q, want OK", stdout.String())
	}
	if stderr.Len() != 0 {
		t.Fatalf("stderr = %q, want empty", stderr.String())
	}
}

func TestValidate_RejectsMissingVerification(t *testing.T) {
	plan := validPlan()
	plan.Verification = nil

	planPath := writeTempPlanJSON(t, mustJSON(t, plan))
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	code := run([]string{"validate", planPath}, stdout, stderr)

	if code != 1 {
		t.Fatalf("run() code = %d, want 1", code)
	}
	if stdout.Len() != 0 {
		t.Fatalf("stdout = %q, want empty", stdout.String())
	}
	if !strings.Contains(stderr.String(), "verification is required") {
		t.Fatalf("stderr = %q, want verification error", stderr.String())
	}
}

func TestCreatePlan_BadUsage_ReturnsExitTwo(t *testing.T) {
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	code := run([]string{"create"}, stdout, stderr)
	if code != 2 {
		t.Fatalf("run() code = %d, want 2", code)
	}
	if !strings.Contains(stderr.String(), "usage: planner create <plan.json> <output.md>") {
		t.Fatalf("stderr = %q, want usage text", stderr.String())
	}
}

func TestVerifyRenderedText_RejectsMissingRenderedCodeBlockForStep(t *testing.T) {
	plan := validPlan()
	plan.Implementation = append(plan.Implementation, Step{
		Title:   "Second step",
		Summary: "This step should also render code.",
		FileChanges: []FileChange{{
			Filename:    "src/plan.go",
			Explanation: "Schema validation exists here.",
			Language:    "go",
			Code:        "type Plan struct {}",
		}},
	})

	rendered := `# Sample Plan
---

## Overview
---

Short summary.

## Definition of Done
---

Rendered markdown exists.

### Goals
- [ ] Renderer succeeds

### Current State

- Existing prompts are duplicated.

### Module Shape

src/main.go

## Implementation
---

### 1. Render sample
Run the engine on a minimal valid plan.
` + "```go\nfunc main() {}\n```\n" + `

### 2. Second step
This step should also render code.

## Verification
---

### Automated Verification
- [ ] go test ./...

### Manual Verification
- [ ] Open the rendered markdown
`

	err := verifyRenderedText(rendered, plan)
	if err == nil {
		t.Fatal("verifyRenderedText() error = nil, want error")
	}
}

// TestRoundTrip verifies that a plan round-trips cleanly through
// appendPlanSource → extractPlanSource with no data loss.
func TestRoundTrip(t *testing.T) {
	plan := validPlan()
	rendered, err := renderPlan(plan)
	if err != nil {
		t.Fatalf("render: %v", err)
	}
	withAppendix, err := appendPlanSource(rendered, plan)
	if err != nil {
		t.Fatalf("appendPlanSource: %v", err)
	}
	got, err := extractPlanSource(withAppendix)
	if err != nil {
		t.Fatalf("extractPlanSource: %v", err)
	}
	if !reflect.DeepEqual(got, plan) {
		t.Errorf("round-trip mismatch:\n got=%#v\nwant=%#v", got, plan)
	}
}

// TestStepAdd exercises the full CLI path for step add: create a plan file,
// run step add, then extract and assert the step was appended.
func TestStepAdd(t *testing.T) {
	plan := validPlan()
	dir := t.TempDir()
	planPath := filepath.Join(dir, "plan.md")
	if err := createPlanFromStruct(plan, planPath); err != nil {
		t.Fatalf("createPlanFromStruct: %v", err)
	}

	newStep := Step{
		Title:   "Extra Step",
		Summary: "An extra step added via step add.",
		FileChanges: []FileChange{
			{Filename: "extra.go", Explanation: "extra", Language: "go", Code: "package main"},
		},
	}
	stepsData, err := json.Marshal([]Step{newStep})
	if err != nil {
		t.Fatalf("marshal steps: %v", err)
	}
	stepsPath := filepath.Join(dir, "steps.json")
	if err := os.WriteFile(stepsPath, stepsData, 0o644); err != nil {
		t.Fatalf("write steps file: %v", err)
	}

	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	code := run([]string{"create", "step", "add", stepsPath, planPath}, stdout, stderr)
	if code != 0 {
		t.Fatalf("run() code = %d, want 0; stderr=%q", code, stderr.String())
	}
	if stdout.String() != planPath+"\n" {
		t.Fatalf("stdout = %q, want %q", stdout.String(), planPath+"\n")
	}
	if stderr.Len() != 0 {
		t.Fatalf("stderr = %q, want empty", stderr.String())
	}

	mdData, err := os.ReadFile(planPath)
	if err != nil {
		t.Fatalf("read plan file: %v", err)
	}
	got, err := extractPlanSource(string(mdData))
	if err != nil {
		t.Fatalf("extractPlanSource after add: %v", err)
	}
	if len(got.Implementation) != len(plan.Implementation)+1 {
		t.Errorf("expected %d steps after add, got %d", len(plan.Implementation)+1, len(got.Implementation))
	}
	if got.Implementation[len(plan.Implementation)].Title != newStep.Title {
		t.Errorf("appended step title mismatch: got %q want %q", got.Implementation[len(plan.Implementation)].Title, newStep.Title)
	}
}

// TestStepReplace exercises the full CLI path for step replace: create a plan
// file, run step replace, then extract and assert the step list was replaced.
func TestStepReplace(t *testing.T) {
	plan := validPlan()
	dir := t.TempDir()
	planPath := filepath.Join(dir, "plan.md")
	if err := createPlanFromStruct(plan, planPath); err != nil {
		t.Fatalf("createPlanFromStruct: %v", err)
	}

	replacement := []Step{{
		Title:   "Replacement Step",
		Summary: "Replaces all steps.",
		FileChanges: []FileChange{
			{Filename: "rep.go", Explanation: "rep", Language: "go", Code: "package main"},
		},
	}}
	stepsData, err := json.Marshal(replacement)
	if err != nil {
		t.Fatalf("marshal steps: %v", err)
	}
	stepsPath := filepath.Join(dir, "steps.json")
	if err := os.WriteFile(stepsPath, stepsData, 0o644); err != nil {
		t.Fatalf("write steps file: %v", err)
	}

	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	code := run([]string{"create", "step", "replace", stepsPath, planPath}, stdout, stderr)
	if code != 0 {
		t.Fatalf("run() code = %d, want 0; stderr=%q", code, stderr.String())
	}
	if stdout.String() != planPath+"\n" {
		t.Fatalf("stdout = %q, want %q", stdout.String(), planPath+"\n")
	}
	if stderr.Len() != 0 {
		t.Fatalf("stderr = %q, want empty", stderr.String())
	}

	mdData, err := os.ReadFile(planPath)
	if err != nil {
		t.Fatalf("read plan file: %v", err)
	}
	got, err := extractPlanSource(string(mdData))
	if err != nil {
		t.Fatalf("extractPlanSource after replace: %v", err)
	}
	if len(got.Implementation) != 1 {
		t.Errorf("expected 1 step after replace, got %d", len(got.Implementation))
	}
	if got.Implementation[0].Title != replacement[0].Title {
		t.Errorf("replaced step title mismatch: got %q want %q", got.Implementation[0].Title, replacement[0].Title)
	}
}

func TestStepAdd_RejectsEmptySteps(t *testing.T) {
	plan := validPlan()
	dir := t.TempDir()
	planPath := filepath.Join(dir, "plan.md")
	if err := createPlanFromStruct(plan, planPath); err != nil {
		t.Fatalf("createPlanFromStruct: %v", err)
	}
	stepsPath := filepath.Join(dir, "steps.json")
	if err := os.WriteFile(stepsPath, []byte("[]"), 0o644); err != nil {
		t.Fatalf("write steps file: %v", err)
	}

	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	code := run([]string{"create", "step", "add", stepsPath, planPath}, stdout, stderr)
	if code != 1 {
		t.Fatalf("run() code = %d, want 1", code)
	}
	if stdout.Len() != 0 {
		t.Fatalf("stdout = %q, want empty", stdout.String())
	}
	if !strings.Contains(stderr.String(), "step add: steps.json must contain at least one step") {
		t.Fatalf("stderr = %q, want empty-step error", stderr.String())
	}
}

func TestRunStep_BadUsage_ReturnsExitTwo(t *testing.T) {
	stdout := new(bytes.Buffer)
	stderr := new(bytes.Buffer)
	code := run([]string{"create", "step"}, stdout, stderr)
	if code != 2 {
		t.Fatalf("run() code = %d, want 2", code)
	}
	if !strings.Contains(stderr.String(), "usage: planner create step <add|replace> <steps.json> <plan.md>") {
		t.Fatalf("stderr = %q, want step usage text", stderr.String())
	}
}

func TestExtractPlanSourceMissingAppendix(t *testing.T) {
	_, err := extractPlanSource("# Some Plan\n\nNo appendix here.")
	if err == nil {
		t.Error("expected error for missing appendix, got nil")
	}
}

func TestCreatePlanFromStructOutputContainsAppendix(t *testing.T) {
	plan := validPlan()
	dir := t.TempDir()
	planPath := filepath.Join(dir, "plan.md")
	if err := createPlanFromStruct(plan, planPath); err != nil {
		t.Fatalf("createPlanFromStruct: %v", err)
	}
	data, err := os.ReadFile(planPath)
	if err != nil {
		t.Fatalf("read plan file: %v", err)
	}
	if !strings.Contains(string(data), "# Appendix") {
		t.Error("rendered plan missing # Appendix section")
	}
	if !strings.Contains(string(data), "## Plan JSON") {
		t.Error("rendered plan missing ## Plan JSON section")
	}
	if strings.Count(string(data), "<!-- planner:source-begin -->") != 1 {
		t.Error("expected exactly one planner:source-begin sentinel")
	}
}

// Regression: a plan whose FileChange.Code contains a triple-backtick fence
// must (a) round-trip cleanly via the appendix and (b) render with a fence
// long enough that the inner run does not close the outer block.
func TestRoundTripWithCodeFenceInFileChange(t *testing.T) {
	plan := validPlan()
	plan.Implementation[0].FileChanges[0].Code = "example:\n```go\nfunc x() {}\n```\n"
	dir := t.TempDir()
	planPath := filepath.Join(dir, "plan.md")
	if err := createPlanFromStruct(plan, planPath); err != nil {
		t.Fatalf("createPlanFromStruct: %v", err)
	}
	data, err := os.ReadFile(planPath)
	if err != nil {
		t.Fatalf("read plan file: %v", err)
	}

	got, err := extractPlanSource(string(data))
	if err != nil {
		t.Fatalf("extractPlanSource: %v", err)
	}
	if got.Implementation[0].FileChanges[0].Code != plan.Implementation[0].FileChanges[0].Code {
		t.Error("FileChange.Code mismatch after round-trip with embedded fence")
	}

	if !strings.Contains(string(data), "````go\n") {
		t.Error("rendered body should use a 4-backtick fence when Code contains a 3-backtick run")
	}
}

// Regression: a plan whose FileChange.Code literally contains the begin/end
// sentinel strings must still round-trip. SetEscapeHTML(true) escapes '<' as
// \u003c so the sentinel cannot appear inside any JSON string value.
func TestRoundTripWithSentinelInFileChangeCode(t *testing.T) {
	plan := validPlan()
	plan.Implementation[0].FileChanges[0].Code = "// <!-- planner:source-begin -->\n// <!-- planner:source-end -->\n"
	dir := t.TempDir()
	planPath := filepath.Join(dir, "plan.md")
	if err := createPlanFromStruct(plan, planPath); err != nil {
		t.Fatalf("createPlanFromStruct: %v", err)
	}
	data, err := os.ReadFile(planPath)
	if err != nil {
		t.Fatalf("read plan file: %v", err)
	}
	got, err := extractPlanSource(string(data))
	if err != nil {
		t.Fatalf("extractPlanSource: %v", err)
	}
	if got.Implementation[0].FileChanges[0].Code != plan.Implementation[0].FileChanges[0].Code {
		t.Error("FileChange.Code mismatch after round-trip with embedded sentinel")
	}
}

// TestCodeFence pins the fence-length contract: N backticks in code requires
// a fence of N+1, minimum 3.
func TestCodeFence(t *testing.T) {
	cases := []struct{ in, want string }{
		{"no backticks", "```"},
		{"one ` backtick", "```"},
		{"two `` backticks", "```"},
		{"three ``` backticks", "````"},
		{"four ```` backticks", "`````"},
	}
	for _, c := range cases {
		if got := codeFence(c.in); got != c.want {
			t.Errorf("codeFence(%q) = %q, want %q", c.in, got, c.want)
		}
	}
}

func mustJSON(t *testing.T, plan Plan) string {
	t.Helper()
	raw, err := json.MarshalIndent(plan, "", "  ")
	if err != nil {
		t.Fatalf("marshal plan: %v", err)
	}
	return string(raw)
}

func writeTempPlanJSON(t *testing.T, body string) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "plan.json")
	if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
		t.Fatalf("write plan file: %v", err)
	}
	return path
}

func mustReadFile(t *testing.T, path string) string {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read file %s: %v", path, err)
	}
	return string(raw)
}

func validPlan() Plan {
	return Plan{
		Title:    "Sample Plan",
		Overview: "Short summary.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Rendered markdown exists.",
			Goals:        []string{"Renderer succeeds"},
			CurrentState: "- Existing prompts are duplicated.",
			ModuleShape:  "src/main.go",
		},
		Implementation: []Step{{
			Title:   "Render sample",
			Summary: "Run the engine on a minimal valid plan.",
			FileChanges: []FileChange{{
				Filename:    "src/main.go",
				Explanation: "CLI entrypoint renders a plan.",
				Language:    "go",
				Code:        "func main() {}",
			}},
		}},
		Verification: &Verification{
			Summary:   "A minimal plan should render cleanly.",
			Automated: []string{"go test ./..."},
			Manual:    []string{"Open the rendered markdown"},
		},
	}
}

func containsAny(items []any, want string) bool {
	for _, item := range items {
		if item == want {
			return true
		}
	}
	return false
}
