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

func TestRunRejectsImplementationZero(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := filepath.Join(tmp, "source.md")
	patchPath := filepath.Join(tmp, "patch.json")
	outputPath := filepath.Join(tmp, "out.md")

	plan := twoStepPlan()
	markdown, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(markdown), 0o644); err != nil {
		t.Fatal(err)
	}

	patch := schema.Step{
		Title:   "New",
		Summary: "new",
		FileChanges: []schema.FileChange{{
			Filename:    "x.go",
			Explanation: "why",
			Diff:        "@@ -1 +1 @@\n-old\n+new",
		}},
	}
	patchJSON, err := json.Marshal(patch)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(patchPath, patchJSON, 0o644); err != nil {
		t.Fatal(err)
	}

	if _, err := Run(sourcePath, "implementation.0", patchPath, outputPath); err == nil {
		t.Fatal("expected error for implementation.0")
	}
}

func TestRunReplacesOnlyRequestedStep(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := filepath.Join(tmp, "source.md")
	patchPath := filepath.Join(tmp, "patch.json")
	outputPath := filepath.Join(tmp, "out.md")

	plan := twoStepPlan()
	markdown, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(markdown), 0o644); err != nil {
		t.Fatal(err)
	}

	patch := schema.Step{
		Title:   "First Updated",
		Summary: "Summary1 updated",
		FileChanges: []schema.FileChange{{
			Filename:    "a.go",
			Explanation: "why",
			Diff:        "@@ -1 +1 @@\n-old\n+newer",
		}},
	}
	patchJSON, err := json.Marshal(patch)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(patchPath, patchJSON, 0o644); err != nil {
		t.Fatal(err)
	}

	contract, err := Run(sourcePath, "implementation.1", patchPath, outputPath)
	if err != nil {
		t.Fatalf("Run: %v", err)
	}
	if len(contract.StepsReplaced) != 1 || contract.StepsReplaced[0] != 1 {
		t.Fatalf("unexpected steps replaced: %+v", contract.StepsReplaced)
	}

	out, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatal(err)
	}
	parsed, _, _, err := inspect.ParseMarkdown(string(out))
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	if !reflect.DeepEqual(parsed.Implementation[0], patch) {
		t.Fatalf("step 1 mismatch: %#v", parsed.Implementation[0])
	}
	if !reflect.DeepEqual(parsed.Implementation[1], plan.Implementation[1]) {
		t.Fatalf("step 2 changed unexpectedly: %#v", parsed.Implementation[1])
	}
}

func TestRunReplacesWholeImplementation(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := filepath.Join(tmp, "source.md")
	patchPath := filepath.Join(tmp, "patch.json")
	outputPath := filepath.Join(tmp, "out.md")

	plan := twoStepPlan()
	markdown, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(markdown), 0o644); err != nil {
		t.Fatal(err)
	}

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
	patchJSON, err := json.Marshal(patch)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(patchPath, patchJSON, 0o644); err != nil {
		t.Fatal(err)
	}

	if _, err := Run(sourcePath, "implementation", patchPath, outputPath); err != nil {
		t.Fatalf("Run: %v", err)
	}

	out, err := os.ReadFile(outputPath)
	if err != nil {
		t.Fatal(err)
	}
	parsed, _, _, err := inspect.ParseMarkdown(string(out))
	if err != nil {
		t.Fatalf("ParseMarkdown: %v", err)
	}
	if !reflect.DeepEqual(parsed.Implementation, patch) {
		t.Fatalf("implementation replacement did not apply: %#v", parsed.Implementation)
	}
	if parsed.Overview != plan.Overview || !reflect.DeepEqual(parsed.DefinitionOfDone, plan.DefinitionOfDone) || !reflect.DeepEqual(parsed.Verification, plan.Verification) {
		t.Fatalf("non-implementation content changed unexpectedly")
	}
}

// TestSpliceOutputMatchesRerender verifies splice produces byte-identical
// output to re-rendering the parsed result. Catches formatting drift like
// dropped blank lines between sections.
func TestSpliceOutputMatchesRerender(t *testing.T) {
	tmp := t.TempDir()
	sourcePath := filepath.Join(tmp, "source.md")
	patchPath := filepath.Join(tmp, "patch.json")
	outputPath := filepath.Join(tmp, "out.md")

	plan := twoStepPlan()
	markdown, err := render.RenderPlan(plan)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(sourcePath, []byte(markdown), 0o644); err != nil {
		t.Fatal(err)
	}

	// Replace just step 1
	patch := schema.Step{
		Title:   "Updated First",
		Summary: "updated summary",
		FileChanges: []schema.FileChange{{
			Filename:    "a.go",
			Explanation: "why",
			Diff:        "@@ -1 +1 @@\n-old\n+new",
		}},
	}
	patchJSON, _ := json.Marshal(patch)
	if err := os.WriteFile(patchPath, patchJSON, 0o644); err != nil {
		t.Fatal(err)
	}

	if _, err := Run(sourcePath, "implementation.1", patchPath, outputPath); err != nil {
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
