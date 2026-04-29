package validate

import (
	"errors"
	"fmt"
	"os"
	"planner/schema"
	"strings"
	"unicode/utf8"
)

// ValidationError records one violation from the aggregate walker.
type ValidationError struct {
	Field   string
	Message string
	Actual  int
}

func (e ValidationError) Error() string { return e.Message }

func ValidatePlan(plan schema.Plan) error {
	if errs := walkValidationRules(plan); len(errs) > 0 {
		return errors.New(errs[0].Message)
	}
	return nil
}

// ValidatePlanAll walks every rule and returns every violation.
func ValidatePlanAll(plan schema.Plan) []ValidationError {
	return walkValidationRules(plan)
}

func walkValidationRules(plan schema.Plan) []ValidationError {
	var errs []ValidationError
	add := func(field, message string, actual int) {
		errs = append(errs, ValidationError{Field: field, Message: message, Actual: actual})
	}

	if strings.TrimSpace(plan.Title) == "" {
		add("title", "title is required", 0)
	} else if got := utf8.RuneCountInString(plan.Title); got > schema.MaxTitleLength {
		add("title", fmt.Sprintf("title must be no more than %d characters (got %d)", schema.MaxTitleLength, got), got)
	}
	if strings.TrimSpace(plan.Overview) == "" {
		add("overview", "overview is required", 0)
	} else if got := utf8.RuneCountInString(plan.Overview); got > schema.MaxOverviewLength {
		add("overview", fmt.Sprintf("overview must be no more than %d characters (got %d)", schema.MaxOverviewLength, got), got)
	}

	dod := plan.DefinitionOfDone
	if strings.TrimSpace(dod.Narrative) == "" {
		add("definition_of_done.narrative", "definition_of_done.narrative is required", 0)
	} else if got := utf8.RuneCountInString(dod.Narrative); got > schema.MaxDoDNarrativeLength {
		add("definition_of_done.narrative", fmt.Sprintf("definition_of_done.narrative must be no more than %d characters (got %d)", schema.MaxDoDNarrativeLength, got), got)
	}
	if len(dod.Goals) == 0 {
		add("definition_of_done.goals", "at least one definition_of_done goal is required", 0)
	} else if got := len(dod.Goals); got > schema.MaxDoDGoals {
		add("definition_of_done.goals", fmt.Sprintf("definition_of_done.goals must have no more than %d goals (got %d)", schema.MaxDoDGoals, got), got)
	}
	for i, goal := range dod.Goals {
		field := fmt.Sprintf("definition_of_done.goals[%d]", i)
		if strings.TrimSpace(goal.Text) == "" {
			add(field+".text", fmt.Sprintf("%s.text is required", field), 0)
		}
		if !validStatus(goal.Status) {
			add(field+".status", fmt.Sprintf("%s.status %q is invalid", field, goal.Status), 0)
		}
		if got := utf8.RuneCountInString(goal.Text); got > schema.MaxDoDGoalLength {
			add(field, fmt.Sprintf("%s must be no more than %d characters (got %d)", field, schema.MaxDoDGoalLength, got), got)
		}
	}
	if strings.TrimSpace(dod.CurrentState) == "" {
		add("definition_of_done.current_state", "definition_of_done.current_state is required", 0)
	} else if got := utf8.RuneCountInString(dod.CurrentState); got > schema.MaxCurrentStateLength {
		add("definition_of_done.current_state", fmt.Sprintf("definition_of_done.current_state must be no more than %d characters (got %d)", schema.MaxCurrentStateLength, got), got)
	}
	if strings.TrimSpace(dod.ModuleShape) == "" {
		add("definition_of_done.module_shape", "definition_of_done.module_shape is required", 0)
	} else {
		for i, line := range strings.Split(dod.ModuleShape, "\n") {
			if got := utf8.RuneCountInString(line); got > schema.MaxModuleShapeLineLength {
				add(fmt.Sprintf("definition_of_done.module_shape[line %d]", i+1), fmt.Sprintf("definition_of_done.module_shape line %d must be no more than %d characters (got %d)", i+1, schema.MaxModuleShapeLineLength, got), got)
			}
		}
	}
	if len(plan.Implementation) == 0 {
		add("implementation", "at least one implementation step is required", 0)
	}
	if plan.Verification == nil {
		add("verification", "verification is required", 0)
	} else {
		for i, item := range plan.Verification.Automated {
			field := fmt.Sprintf("verification.automated[%d]", i)
			if strings.TrimSpace(item.Text) == "" {
				add(field+".text", fmt.Sprintf("%s.text is required", field), 0)
			} else if got := utf8.RuneCountInString(item.Text); got > schema.MaxVerificationItemTextLength {
				add(field+".text", fmt.Sprintf("%s.text must be no more than %d characters (got %d)", field, schema.MaxVerificationItemTextLength, got), got)
			}
			if !validStatus(item.Status) {
				add(field+".status", fmt.Sprintf("%s.status %q is invalid", field, item.Status), 0)
			}
		}
		for i, item := range plan.Verification.Manual {
			field := fmt.Sprintf("verification.manual[%d]", i)
			if strings.TrimSpace(item.Text) == "" {
				add(field+".text", fmt.Sprintf("%s.text is required", field), 0)
			} else if got := utf8.RuneCountInString(item.Text); got > schema.MaxVerificationItemTextLength {
				add(field+".text", fmt.Sprintf("%s.text must be no more than %d characters (got %d)", field, schema.MaxVerificationItemTextLength, got), got)
			}
			if !validStatus(item.Status) {
				add(field+".status", fmt.Sprintf("%s.status %q is invalid", field, item.Status), 0)
			}
		}
	}

	for i, step := range plan.Implementation {
		field := fmt.Sprintf("implementation[%d]", i)
		if strings.TrimSpace(step.Title) == "" {
			add(field+".title", "each implementation step needs a title", 0)
		} else if got := utf8.RuneCountInString(step.Title); got > schema.MaxStepTitleLength {
			add(field+".title", fmt.Sprintf("%s.title must be no more than %d characters (got %d)", field, schema.MaxStepTitleLength, got), got)
		}
		if strings.TrimSpace(step.Summary) == "" {
			add(field+".summary", "each implementation step needs a summary", 0)
		} else if got := utf8.RuneCountInString(step.Summary); got > schema.MaxStepSummaryLength {
			add(field+".summary", fmt.Sprintf("%s.summary must be no more than %d characters (got %d)", field, schema.MaxStepSummaryLength, got), got)
		}
		if len(step.FileChanges) == 0 {
			add(field+".file_changes", "each implementation step needs file changes", 0)
		}
		for j, change := range step.FileChanges {
			changeField := fmt.Sprintf("%s.file_changes[%d]", field, j)
			if err := schema.ValidateFilenameShape(change.Filename); err != nil {
				add(changeField+".filename", err.Error(), 0)
			}
			if strings.TrimSpace(change.Explanation) == "" {
				add(changeField+".explanation", "each file change needs an explanation", 0)
			} else if got := utf8.RuneCountInString(change.Explanation); got > schema.MaxFileChangeExplanationLength {
				add(changeField+".explanation", fmt.Sprintf("%s.explanation must be no more than %d characters (got %d)", changeField, schema.MaxFileChangeExplanationLength, got), got)
			}
			if strings.TrimSpace(change.Diff) == "" {
				add(changeField+".diff", "each file change needs a diff", 0)
			}
		}
	}

	return errs
}

// GetCodeFence returns the shortest backtick fence that safely wraps content.
// Always at least three backticks; longer if content contains backtick runs.
func GetCodeFence(code string) string {
	longest := 0
	cur := 0
	for _, r := range code {
		if r == '`' {
			cur++
			if cur > longest {
				longest = cur
			}
		} else {
			cur = 0
		}
	}
	n := longest + 1
	if n < 3 {
		n = 3
	}
	return strings.Repeat("`", n)
}

func VerifyRenderedText(rendered string, plan schema.Plan) error {
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
			return fmt.Errorf("missing section: %s", section)
		}
	}

	if !strings.Contains(rendered, "### 1.") {
		return errors.New("missing numbered implementation step")
	}

	for i, step := range plan.Implementation {
		heading := fmt.Sprintf("### %d. %s", i+1, step.Title)
		if !strings.Contains(rendered, heading) {
			return fmt.Errorf("missing rendered implementation step: %s", heading)
		}
		for _, change := range step.FileChanges {
			fence := GetCodeFence(change.Diff)
			block := fence + "diff\n" + change.Diff + "\n" + fence
			if !strings.Contains(rendered, block) {
				return fmt.Errorf("missing rendered code block for %s", change.Filename)
			}
		}
	}

	for _, goal := range plan.DefinitionOfDone.Goals {
		marker := "- [ ] "
		if goal.IsDone() {
			marker = "- [x] "
		}
		if !strings.Contains(rendered, marker+goal.Text) {
			return fmt.Errorf("missing rendered goal: %q", goal.Text)
		}
	}
	if plan.Verification != nil {
		for _, item := range plan.Verification.Automated {
			marker := "- [ ] "
			if item.IsDone() {
				marker = "- [x] "
			}
			if !strings.Contains(rendered, marker+item.Text) {
				return fmt.Errorf("missing rendered automated check: %q", item.Text)
			}
		}
		for _, item := range plan.Verification.Manual {
			marker := "- [ ] "
			if item.IsDone() {
				marker = "- [x] "
			}
			if !strings.Contains(rendered, marker+item.Text) {
				return fmt.Errorf("missing rendered manual check: %q", item.Text)
			}
		}
	}
	return nil
}

func ReadPlanFile(path string) (schema.Plan, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return schema.Plan{}, err
	}
	return schema.DecodePlan(data)
}

// validStatus is the single source of truth for allowed runtime statuses.
// Unchecked items are represented as empty status; pending is normalized away
// at the JSON boundary.
func validStatus(s schema.ChecklistStatus) bool {
	return s == "" || s == schema.StatusDone
}
