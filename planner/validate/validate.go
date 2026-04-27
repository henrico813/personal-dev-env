package validate

import (
	"errors"
	"fmt"
	"os"
	"planner/schema"
	"strings"
	"unicode/utf8"
)

const (
	maxDefinitionOfDoneGoals      = 8
	maxDefinitionOfDoneGoalLength = 88
)

func ValidatePlan(plan schema.Plan) error {
	if strings.TrimSpace(plan.Title) == "" {
		return errors.New("title is required")
	}
	if strings.TrimSpace(plan.Overview) == "" {
		return errors.New("overview is required")
	}
	if strings.TrimSpace(plan.DefinitionOfDone.Narrative) == "" {
		return errors.New("definition_of_done.narrative is required")
	}
	if len(plan.DefinitionOfDone.Goals) == 0 {
		return errors.New("at least one definition_of_done goal is required")
	}
	if len(plan.DefinitionOfDone.Goals) > maxDefinitionOfDoneGoals {
		return fmt.Errorf("definition_of_done.goals must have no more than %d goals", maxDefinitionOfDoneGoals)
	}
	for i, goal := range plan.DefinitionOfDone.Goals {
		if strings.TrimSpace(goal.Text) == "" {
			return fmt.Errorf("definition_of_done.goals[%d].text is required", i)
		}
		if !validStatus(goal.Status) {
			return fmt.Errorf("definition_of_done.goals[%d].status %q is invalid", i, goal.Status)
		}
		if utf8.RuneCountInString(goal.Text) > maxDefinitionOfDoneGoalLength {
			return fmt.Errorf(
				"definition_of_done.goals[%d] must be no more than %d characters",
				i,
				maxDefinitionOfDoneGoalLength,
			)
		}
	}
	if strings.TrimSpace(plan.DefinitionOfDone.CurrentState) == "" {
		return errors.New("definition_of_done.current_state is required")
	}
	if strings.TrimSpace(plan.DefinitionOfDone.ModuleShape) == "" {
		return errors.New("definition_of_done.module_shape is required")
	}
	if len(plan.Implementation) == 0 {
		return errors.New("at least one implementation step is required")
	}
	if plan.Verification == nil {
		return errors.New("verification is required")
	}
	for i, item := range plan.Verification.Automated {
		if strings.TrimSpace(item.Text) == "" {
			return fmt.Errorf("verification.automated[%d].text is required", i)
		}
		if !validStatus(item.Status) {
			return fmt.Errorf("verification.automated[%d].status %q is invalid", i, item.Status)
		}
	}
	for i, item := range plan.Verification.Manual {
		if strings.TrimSpace(item.Text) == "" {
			return fmt.Errorf("verification.manual[%d].text is required", i)
		}
		if !validStatus(item.Status) {
			return fmt.Errorf("verification.manual[%d].status %q is invalid", i, item.Status)
		}
	}

	for _, step := range plan.Implementation {
		if strings.TrimSpace(step.Title) == "" {
			return errors.New("each implementation step needs a title")
		}
		if strings.TrimSpace(step.Summary) == "" {
			return errors.New("each implementation step needs a summary")
		}
		if len(step.FileChanges) == 0 {
			return errors.New("each implementation step needs file changes")
		}
		for _, change := range step.FileChanges {
			if err := schema.ValidateFilenameShape(change.Filename); err != nil {
				return err
			}
			if strings.TrimSpace(change.Explanation) == "" {
				return errors.New("each file change needs an explanation")
			}
			if strings.TrimSpace(change.Diff) == "" {
				return errors.New("each file change needs a diff")
			}
		}
	}

	return nil
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
