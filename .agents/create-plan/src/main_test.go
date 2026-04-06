package main

import (
	"strings"
	"testing"
)

func TestWriteTextIncludesRequiredSections(t *testing.T) {
	tmpl := readTemplate()

	plan := validPlan()
	rendered := writeText(tmpl, plan)

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
			t.Fatalf("missing section %q", section)
		}
	}

	verifyText(rendered, plan)
}

func TestValidatePlanRejectsMissingImplementation(t *testing.T) {
	expectPanic(t, func() {
		validatePlan(Plan{Title: "bad", Overview: "missing steps"})
	})
}

func TestValidatePlanRejectsMissingNarrative(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Narrative = ""
	expectPanic(t, func() {
		validatePlan(plan)
	})
}

func TestValidatePlanRejectsMissingGoals(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.Goals = nil
	expectPanic(t, func() {
		validatePlan(plan)
	})
}

func TestValidatePlanRejectsMissingCurrentState(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.CurrentState = ""
	expectPanic(t, func() {
		validatePlan(plan)
	})
}

func TestValidatePlanRejectsMissingModuleShape(t *testing.T) {
	plan := validPlan()
	plan.DefinitionOfDone.ModuleShape = ""
	expectPanic(t, func() {
		validatePlan(plan)
	})
}

func TestValidatePlanRejectsMissingStepSummary(t *testing.T) {
	plan := validPlan()
	plan.Implementation[0].Summary = ""
	expectPanic(t, func() {
		validatePlan(plan)
	})
}

func TestValidatePlanRejectsMissingFilename(t *testing.T) {
	plan := validPlan()
	plan.Implementation[0].FileChanges[0].Filename = ""
	expectPanic(t, func() {
		validatePlan(plan)
	})
}

func TestValidatePlanRejectsMissingCode(t *testing.T) {
	plan := validPlan()
	plan.Implementation[0].FileChanges[0].Code = ""
	expectPanic(t, func() {
		validatePlan(plan)
	})
}

func TestVerifyTextRejectsMissingRenderedCodeBlockForStep(t *testing.T) {
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

	expectPanic(t, func() {
		verifyText(rendered, plan)
	})
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
		Verification: Verification{
			Summary:   "A minimal plan should render cleanly.",
			Automated: []string{"go test ./..."},
			Manual:    []string{"Open the rendered markdown"},
		},
	}
}

func expectPanic(t *testing.T, fn func()) {
	t.Helper()

	defer func() {
		if recover() == nil {
			t.Fatal("expected panic")
		}
	}()

	fn()
}
