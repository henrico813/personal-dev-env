package replace

import (
	"encoding/json"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"planner/inspect"
	"planner/internal/jsoninput"
	"planner/render"
	"planner/schema"
)

// writeRenderedPlan renders plan to a temp file and returns its path.
func writeRenderedPlan(t *testing.T, plan schema.Plan) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "source.md")
	markdown, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	if err := os.WriteFile(path, []byte(markdown), 0o644); err != nil {
		t.Fatalf("WriteFile(source): %v", err)
	}
	return path
}

// writePatchJSON marshals value to a temp file and returns its path.
func writePatchJSON(t *testing.T, value any) string {
	t.Helper()
	path := filepath.Join(t.TempDir(), "patch.json")
	raw, err := json.Marshal(value)
	if err != nil {
		t.Fatalf("json.Marshal: %v", err)
	}
	if err := os.WriteFile(path, raw, 0o644); err != nil {
		t.Fatalf("WriteFile(patch): %v", err)
	}
	return path
}

// parseOutputPlan reads outputPath and parses it via inspect.ParseMarkdown.
func parseOutputPlan(t *testing.T, outputPath string) schema.Plan {
	t.Helper()
	raw, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("ReadFile(output): %v", err)
	}
	parsed, _, _, _, err := inspect.ParseMarkdown(string(raw))
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	return parsed
}

func TestRunRejectsInvalidSection(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := writeRenderedPlan(t, twoStepPlan())
	patchPath := writePatchJSON(t, "x")
	outputPath := filepath.Join(tmp, "out.md")

	if _, err := Run(sourcePath, ReplaceOptions{Section: "bogus"}, patchPath, outputPath); err == nil {
		t.Fatal("expected invalid section error")
	}
}

func TestRunRejectsInvalidStepIndex(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := writeRenderedPlan(t, twoStepPlan())
	patchPath := writePatchJSON(t, schema.Step{
		Title:   "New",
		Summary: "new",
		FileChanges: []schema.FileChange{{
			Filename:    "x.go",
			Explanation: "why",
			Diff:        "@@ -1 +1 @@\n-old\n+new",
		}},
	})
	outputPath := filepath.Join(tmp, "out.md")

	if _, err := Run(sourcePath, ReplaceOptions{Section: "implementation", Subsection: "0"}, patchPath, outputPath); err == nil {
		t.Fatal("expected invalid step index error")
	}
}

func TestRunRejectsAppendWithSubsection(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := writeRenderedPlan(t, twoStepPlan())
	patchPath := writePatchJSON(t, schema.Step{
		Title:   "New",
		Summary: "new",
		FileChanges: []schema.FileChange{{
			Filename:    "x.go",
			Explanation: "why",
			Diff:        "@@ -1 +1 @@\n-old\n+new",
		}},
	})
	outputPath := filepath.Join(tmp, "out.md")

	if _, err := Run(sourcePath, ReplaceOptions{Section: "implementation", Subsection: "1", Append: true}, patchPath, outputPath); err == nil {
		t.Fatal("expected append/subsection validation error")
	}
}

func TestRunReplacesOnlyRequestedStep(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := writeRenderedPlan(t, twoStepPlan())
	patch := schema.Step{
		Title:   "First Updated",
		Summary: "Summary1 updated",
		FileChanges: []schema.FileChange{{
			Filename:    "a.go",
			Explanation: "why",
			Diff:        "@@ -1 +1 @@\n-old\n+newer",
		}},
	}
	patchPath := writePatchJSON(t, patch)
	outputPath := filepath.Join(tmp, "out.md")

	result, err := Run(sourcePath, ReplaceOptions{Section: "implementation", Subsection: "1"}, patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if len(result.StepsReplaced) != 1 || result.StepsReplaced[0] != 1 {
		t.Fatalf("unexpected steps replaced: %+v", result.StepsReplaced)
	}

	parsed := parseOutputPlan(t, outputPath)
	if !reflect.DeepEqual(parsed.Implementation[0], patch) {
		t.Fatalf("step 1 mismatch: %#v", parsed.Implementation[0])
	}
	if !reflect.DeepEqual(parsed.Implementation[1], twoStepPlan().Implementation[1]) {
		t.Fatalf("step 2 changed unexpectedly: %#v", parsed.Implementation[1])
	}
}

func TestRunReplacesWholeImplementation(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := writeRenderedPlan(t, twoStepPlan())
	patch := []schema.Step{
		{
			Title:   "Replaced One",
			Summary: "step one updated",
			FileChanges: []schema.FileChange{{
				Filename:    "a.go",
				Explanation: "why",
				Diff:        "@@ -1 +1 @@\n-old\n+new",
			}},
		},
		{
			Title:   "Replaced Two",
			Summary: "step two updated",
			FileChanges: []schema.FileChange{{
				Filename:    "b.go",
				Explanation: "why",
				Diff:        "@@ -1 +1 @@\n-old\n+new",
			}},
		},
	}
	patchPath := writePatchJSON(t, patch)
	outputPath := filepath.Join(tmp, "out.md")

	if _, err := Run(sourcePath, ReplaceOptions{Section: "implementation"}, patchPath, outputPath); err != nil {
		t.Fatalf("Run: %v", err)
	}

	parsed := parseOutputPlan(t, outputPath)
	if !reflect.DeepEqual(parsed.Implementation, patch) {
		t.Fatalf("implementation replacement did not apply: %#v", parsed.Implementation)
	}
	plan := twoStepPlan()
	if parsed.Overview != plan.Overview || !reflect.DeepEqual(parsed.DefinitionOfDone, plan.DefinitionOfDone) || !reflect.DeepEqual(parsed.Verification, plan.Verification) {
		t.Fatalf("non-implementation content changed unexpectedly")
	}
}

func TestRunReplacesOverview(t *testing.T) {
	tmp := t.TempDir()
	plan := twoStepPlan()
	sourcePath := writeRenderedPlan(t, plan)
	patchPath := writePatchJSON(t, "Updated overview")
	outputPath := filepath.Join(tmp, "out.md")

	result, err := Run(sourcePath, ReplaceOptions{Section: "overview"}, patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if result.Section != "overview" || result.Appended {
		t.Fatalf("unexpected replace result: %#v", result)
	}

	parsed := parseOutputPlan(t, outputPath)
	if parsed.Overview != "Updated overview" {
		t.Fatalf("overview mismatch: %q", parsed.Overview)
	}
	if !reflect.DeepEqual(parsed.Implementation, plan.Implementation) {
		t.Fatal("implementation changed unexpectedly")
	}
}

func TestRunReplacesDefinitionOfDoneSubsection(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := writeRenderedPlan(t, twoStepPlan())
	patchPath := writePatchJSON(t, "Updated shape")
	outputPath := filepath.Join(tmp, "out.md")

	result, err := Run(sourcePath, ReplaceOptions{Section: "definition_of_done", Subsection: "module_shape"}, patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if result.Subsection != "module_shape" {
		t.Fatalf("unexpected replace result: %#v", result)
	}

	parsed := parseOutputPlan(t, outputPath)
	if parsed.DefinitionOfDone.ModuleShape != "Updated shape" {
		t.Fatalf("module shape mismatch: %q", parsed.DefinitionOfDone.ModuleShape)
	}
}

func TestRunReplacesVerification(t *testing.T) {
	tmp := t.TempDir()
	plan := twoStepPlan()
	sourcePath := writeRenderedPlan(t, plan)
	patchPath := writePatchJSON(t, schema.Verification{
		Summary:   "Updated verification",
		Automated: []schema.ChecklistItem{{Text: "go test ./... -run TestRunReplacesVerification"}},
		Manual:    []schema.ChecklistItem{{Text: "smoke"}},
	})
	outputPath := filepath.Join(tmp, "out.md")

	if _, err := Run(sourcePath, ReplaceOptions{Section: "verification"}, patchPath, outputPath); err != nil {
		t.Fatalf("Run: %v", err)
	}

	parsed := parseOutputPlan(t, outputPath)
	if parsed.Verification.Summary != "Updated verification" {
		t.Fatalf("verification mismatch: %#v", parsed.Verification)
	}
	if parsed.Overview != plan.Overview {
		t.Fatal("overview changed unexpectedly")
	}
}

func TestRunAppendsStep(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := writeRenderedPlan(t, twoStepPlan())
	patchPath := writePatchJSON(t, schema.Step{
		Title:   "Third",
		Summary: "Summary3",
		FileChanges: []schema.FileChange{{
			Filename:    "c.go",
			Explanation: "why",
			Diff:        "@@ -1 +1 @@\n-old\n+new",
		}},
	})
	outputPath := filepath.Join(tmp, "out.md")

	result, err := Run(sourcePath, ReplaceOptions{Section: "implementation", Append: true}, patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if !result.Appended || len(result.StepsReplaced) != 1 || result.StepsReplaced[0] != 3 {
		t.Fatalf("unexpected replace result: %#v", result)
	}

	parsed := parseOutputPlan(t, outputPath)
	if len(parsed.Implementation) != 3 {
		t.Fatalf("expected 3 steps, got %d", len(parsed.Implementation))
	}
}

func TestRunAppendsStepToEmptyImplementation(t *testing.T) {
	tmp := t.TempDir()
	plan := twoStepPlan()
	plan.Implementation = nil
	sourcePath := writeRenderedPlan(t, plan)
	patchPath := writePatchJSON(t, schema.Step{
		Title:   "First",
		Summary: "Summary1",
		FileChanges: []schema.FileChange{{
			Filename:    "a.go",
			Explanation: "why",
			Diff:        "@@ -1 +1 @@\n-old\n+new",
		}},
	})
	outputPath := filepath.Join(tmp, "out.md")

	result, err := Run(sourcePath, ReplaceOptions{Section: "implementation", Append: true}, patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if !result.Appended || result.StepsReplaced[0] != 1 {
		t.Fatalf("unexpected replace result: %#v", result)
	}

	parsed := parseOutputPlan(t, outputPath)
	if len(parsed.Implementation) != 1 {
		t.Fatalf("expected 1 step, got %d", len(parsed.Implementation))
	}
}

func TestSpliceDiffFieldHappyPath(t *testing.T) {
	src := writeFixturePlan(t, twoNamedFileChanges("a.go", "OLD A", "b.go", "OLD B"))
	opts := ReplaceOptions{
		Section:    "implementation",
		Subsection: "1",
		File:       "a.go",
		Field:      "diff",
	}

	out, result, err := PreviewFromData(src, opts, []byte("NEW DIFF FOR A"))
	if err != nil {
		t.Fatalf("PreviewFromData: %v", err)
	}
	if !strings.Contains(out, "NEW DIFF FOR A") {
		t.Fatalf("new diff missing from output")
	}
	if strings.Contains(out, "OLD A") {
		t.Fatalf("old diff still present")
	}
	if !strings.Contains(out, "OLD B") {
		t.Fatalf("untargeted diff was disturbed")
	}
	if result.File != "a.go" || result.Field != "diff" {
		t.Fatalf("ReplaceResult missing File/Field: %+v", result)
	}
}

func TestSpliceDiffFieldFileNotFound(t *testing.T) {
	src := writeFixturePlan(t, twoNamedFileChanges("a.go", "OLD A", "b.go", "OLD B"))
	opts := ReplaceOptions{
		Section:    "implementation",
		Subsection: "1",
		File:       "missing.go",
		Field:      "diff",
	}

	_, _, err := PreviewFromData(src, opts, []byte("X"))
	var re *ReplaceError
	if !errors.As(err, &re) || re.Code != ReplaceFileNotFoundError {
		t.Fatalf("got %v, want ReplaceFileNotFoundError", err)
	}
}

func TestSpliceDiffFieldFileAmbiguous(t *testing.T) {
	src := writeFixturePlan(t, twoNamedFileChanges("a.go", "OLD ONE", "a.go", "OLD TWO"))
	opts := ReplaceOptions{
		Section:    "implementation",
		Subsection: "1",
		File:       "a.go",
		Field:      "diff",
	}

	_, _, err := PreviewFromData(src, opts, []byte("X"))
	var re *ReplaceError
	if !errors.As(err, &re) || re.Code != ReplaceFileAmbiguousError {
		t.Fatalf("got %v, want ReplaceFileAmbiguousError", err)
	}
}

func TestSpliceDiffFieldRejectsUnparseableDiff(t *testing.T) {
	src := writeFixturePlan(t, twoNamedFileChanges("a.go", "OLD A", "b.go", "OLD B"))
	opts := ReplaceOptions{
		Section:    "implementation",
		Subsection: "1",
		File:       "a.go",
		Field:      "diff",
	}
	unparseable := []byte("some prefix\n```\nfake fence inside diff\n```\nrest")

	_, _, err := PreviewFromData(src, opts, unparseable)
	var re *ReplaceError
	if !errors.As(err, &re) || re.Code != ReplaceParseSplicedSourceError {
		t.Fatalf("got %v, want ReplaceParseSplicedSourceError", err)
	}
}

// TestSpliceDiffFieldRejectsEmptyDiff guards the integrity invariant that
// successful patch output is still a valid plan. Empty diff bodies parse but
// fail validate.ValidatePlan; the path must reject before write.
func TestSpliceDiffFieldRejectsEmptyDiff(t *testing.T) {
	src := writeFixturePlan(t, twoNamedFileChanges("a.go", "OLD A", "b.go", "OLD B"))
	opts := ReplaceOptions{
		Section:    "implementation",
		Subsection: "1",
		File:       "a.go",
		Field:      "diff",
	}

	for _, tc := range []struct {
		name string
		body []byte
	}{
		{"empty", []byte("")},
		{"whitespace_only", []byte("   \n\t\n")},
	} {
		t.Run(tc.name, func(t *testing.T) {
			_, _, err := PreviewFromData(src, opts, tc.body)
			var re *ReplaceError
			if !errors.As(err, &re) || re.Code != ReplaceValidateResultError {
				t.Fatalf("got %v, want ReplaceValidateResultError", err)
			}
		})
	}
}

// TestSpliceOutputMatchesRerender verifies splice produces byte-identical
// output to re-rendering the parsed result. Catches formatting drift like
// dropped blank lines between sections.
func TestSpliceOutputMatchesRerender(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := writeRenderedPlan(t, twoStepPlan())
	patch := schema.Step{
		Title:   "Updated First",
		Summary: "updated summary",
		FileChanges: []schema.FileChange{{
			Filename:    "a.go",
			Explanation: "why",
			Diff:        "@@ -1 +1 @@\n-old\n+new",
		}},
	}
	patchPath := writePatchJSON(t, patch)
	outputPath := filepath.Join(tmp, "out.md")

	if _, err := Run(sourcePath, ReplaceOptions{Section: "implementation", Subsection: "1"}, patchPath, outputPath); err != nil {
		t.Fatalf("Run: %v", err)
	}

	spliced, _ := os.ReadFile(outputPath)
	parsed, _, _, _, err := inspect.ParseMarkdown(string(spliced))
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	rerendered, err := render.RenderPlan(parsed)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}

	if string(spliced) != rerendered {
		t.Fatalf("splice output differs from re-rendered plan")
	}
}

func twoStepPlan() schema.Plan {
	return schema.Plan{
		Title:    "Plan",
		Overview: "Overview",
		DefinitionOfDone: schema.DefinitionOfDone{
			Narrative:    "Narrative",
			Goals:        []schema.ChecklistItem{{Text: "Goal"}},
			CurrentState: "Current",
			ModuleShape:  "Shape",
		},
		Implementation: []schema.Step{
			{
				Title:   "First",
				Summary: "Summary1",
				FileChanges: []schema.FileChange{{
					Filename:    "a.go",
					Explanation: "why",
					Diff:        "@@ -1 +1 @@\n-old\n+new",
				}},
			},
			{
				Title:   "Second",
				Summary: "Summary2",
				FileChanges: []schema.FileChange{{
					Filename:    "b.go",
					Explanation: "why",
					Diff:        "@@ -1 +1 @@\n-old\n+new",
				}},
			},
		},
		Verification: &schema.Verification{
			Summary:   "Summary",
			Automated: []schema.ChecklistItem{{Text: "go test ./..."}},
			Manual:    []schema.ChecklistItem{{Text: "smoke"}},
		},
	}
}

func writeFixturePlan(t *testing.T, plan schema.Plan) string {
	t.Helper()
	return writeRenderedPlan(t, plan)
}

func twoNamedFileChanges(name1, diff1, name2, diff2 string) schema.Plan {
	plan := twoStepPlan()
	plan.Implementation[0].FileChanges = []schema.FileChange{
		{
			Filename:    name1,
			Explanation: "why",
			Diff:        diff1,
		},
		{
			Filename:    name2,
			Explanation: "why",
			Diff:        diff2,
		},
	}
	return plan
}

// TestRunPreservesCheckboxesInUntouchedSectionByteIdentical verifies that
// replacing only the overview section leaves the DoD section byte-identical,
// including Obsidian-style "- [X]" markers that must not be rewritten to
// "- [x]". This guards the untouched-section byte-identity invariant.
func TestReplacePreservesUntouchedSections(t *testing.T) {
	plan := twoStepPlan()
	plan.DefinitionOfDone.Goals = []schema.ChecklistItem{
		{Text: "pending goal"},
		{Text: "done goal", Status: schema.StatusDone},
	}
	md, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	// Simulate Obsidian macOS capitalizing [x] to [X] on check.
	mdWithCapX := strings.ReplaceAll(md, "- [x] done goal", "- [X] done goal")
	if mdWithCapX == md {
		t.Fatal("expected to find - [x] done goal in rendered output")
	}
	dir := t.TempDir()
	src := dir + "/plan.md"
	if err := os.WriteFile(src, []byte(mdWithCapX), 0o644); err != nil {
		t.Fatal(err)
	}
	patchPath := writePatchJSON(t, "New overview.")
	outputPath := dir + "/out.md"
	if _, err := Run(src, ReplaceOptions{Section: "overview"}, patchPath, outputPath); err != nil {
		t.Fatalf("Run: %v", err)
	}
	out, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	outStr := string(out)
	if !strings.Contains(outStr, "- [ ] pending goal") {
		t.Fatalf("pending goal lost in output:\n%s", outStr)
	}
	if !strings.Contains(outStr, "- [X] done goal") {
		t.Fatalf("- [X] rewritten or lost in output (must be byte-identical):\n%s", outStr)
	}
	if strings.Contains(outStr, "- [x] done goal") {
		t.Fatalf("- [x] found: untouched section was re-rendered instead of preserved:\n%s", outStr)
	}
}

func TestGoalsPatchAcceptsLegacyStringsAndObjects(t *testing.T) {
	for _, tc := range []struct {
		name  string
		patch any
	}{
		{"plain_strings", []string{"updated goal"}},
		{"objects", []map[string]string{{"text": "updated goal"}}},
	} {
		t.Run(tc.name, func(t *testing.T) {
			src := writeRenderedPlan(t, twoStepPlan())
			patchPath := writePatchJSON(t, tc.patch)
			outputPath := t.TempDir() + "/out.md"
			if _, err := Run(src, ReplaceOptions{Section: "definition_of_done", Subsection: "goals"}, patchPath, outputPath); err != nil {
				t.Fatalf("Run: %v", err)
			}
			parsed := parseOutputPlan(t, outputPath)
			if len(parsed.DefinitionOfDone.Goals) != 1 || parsed.DefinitionOfDone.Goals[0].Text != "updated goal" {
				t.Fatalf("goals mismatch: %+v", parsed.DefinitionOfDone.Goals)
			}
		})
	}
}

func TestDecodeRejectsTrailingData(t *testing.T) {
	var s string
	if err := jsoninput.DecodeStrict([]byte(`"valid" trailing`), &s); err == nil {
		t.Fatal("expected error for trailing data after JSON value")
	}
}

func TestPreviewFromDataReturnsDecodePatchError(t *testing.T) {
	sourcePath := writeRenderedPlan(t, twoStepPlan())
	_, _, err := PreviewFromData(sourcePath, ReplaceOptions{Section: "overview"}, []byte(`{"not":"a string"}`))
	if err == nil {
		t.Fatal("expected decode patch error")
	}
	var replaceErr *ReplaceError
	if !errors.As(err, &replaceErr) {
		t.Fatalf("expected ReplaceError, got %T", err)
	}
	if replaceErr.Code != ReplaceDecodePatchError {
		t.Fatalf("got code %v, want %v", replaceErr.Code, ReplaceDecodePatchError)
	}
}

func TestPreviewFromDataReturnsParseSourceError(t *testing.T) {
	dir := t.TempDir()
	sourcePath := dir + "/broken.md"
	if err := os.WriteFile(sourcePath, []byte("# not a planner doc\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	_, _, err := PreviewFromData(sourcePath, ReplaceOptions{Section: "overview"}, []byte(`"new overview"`))
	if err == nil {
		t.Fatal("expected parse source error")
	}
	var replaceErr *ReplaceError
	if !errors.As(err, &replaceErr) {
		t.Fatalf("expected ReplaceError, got %T", err)
	}
	if replaceErr.Code != ReplaceParseSourceError {
		t.Fatalf("got code %v, want %v", replaceErr.Code, ReplaceParseSourceError)
	}
}

func TestPreviewPreservesCheckboxesInUntouchedSections(t *testing.T) {
	plan := twoStepPlan()
	plan.DefinitionOfDone.Goals = []schema.ChecklistItem{
		{Text: "pending goal"},
		{Text: "done goal", Status: schema.StatusDone},
	}
	sourcePath := writeRenderedPlan(t, plan)

	out, _, err := PreviewFromData(sourcePath, ReplaceOptions{Section: "overview"}, []byte(`"New overview."`))
	if err != nil {
		t.Fatalf("PreviewFromData: %v", err)
	}
	if !strings.Contains(out, "- [ ] pending goal") {
		t.Fatalf("lost pending checkbox state:\n%s", out)
	}
	if !strings.Contains(out, "- [x] done goal") {
		t.Fatalf("lost done checkbox state:\n%s", out)
	}
}
