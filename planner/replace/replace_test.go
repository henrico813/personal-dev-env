package replace

import (
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"planner/inspect"
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
	parsed, _, _, err := inspect.ParseMarkdown(string(raw))
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

	contract, err := Run(sourcePath, ReplaceOptions{Section: "implementation", Subsection: "1"}, patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if len(contract.StepsReplaced) != 1 || contract.StepsReplaced[0] != 1 {
		t.Fatalf("unexpected steps replaced: %+v", contract.StepsReplaced)
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

	contract, err := Run(sourcePath, ReplaceOptions{Section: "overview"}, patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if contract.Section != "overview" || contract.Appended {
		t.Fatalf("unexpected contract: %#v", contract)
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

	contract, err := Run(sourcePath, ReplaceOptions{Section: "definition_of_done", Subsection: "module_shape"}, patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if contract.Subsection != "module_shape" {
		t.Fatalf("unexpected contract: %#v", contract)
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
		Automated: []string{"go test ./... -run TestRunReplacesVerification"},
		Manual:    []string{"smoke"},
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

	contract, err := Run(sourcePath, ReplaceOptions{Section: "implementation", Append: true}, patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if !contract.Appended || len(contract.StepsReplaced) != 1 || contract.StepsReplaced[0] != 3 {
		t.Fatalf("unexpected contract: %#v", contract)
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

	contract, err := Run(sourcePath, ReplaceOptions{Section: "implementation", Append: true}, patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if !contract.Appended || contract.StepsReplaced[0] != 1 {
		t.Fatalf("unexpected contract: %#v", contract)
	}

	parsed := parseOutputPlan(t, outputPath)
	if len(parsed.Implementation) != 1 {
		t.Fatalf("expected 1 step, got %d", len(parsed.Implementation))
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
	parsed, _, _, err := inspect.ParseMarkdown(string(spliced))
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
			Goals:        []string{"Goal"},
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
			Automated: []string{"go test ./..."},
			Manual:    []string{"smoke"},
		},
	}
}
