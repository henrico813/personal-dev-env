package validate

import (
	"errors"
	"os"
	"strings"
	"planner/schema"
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
			if strings.TrimSpace(change.Filename) == "" {
				return errors.New("each file change needs a filename")
			}
			if strings.TrimSpace(change.Explanation) == "" {
				return errors.New("each file change needs an explanation")
			}
			if strings.TrimSpace(change.Language) == "" {
				return errors.New("each file change needs a language")
			}
			if strings.TrimSpace(change.Code) == "" {
				return errors.New("each file change needs code")
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
