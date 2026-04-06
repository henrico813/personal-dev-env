package main

import "strings"

type Plan struct {
	Title            string           `json:"title"`
	Overview         string           `json:"overview"`
	DefinitionOfDone DefinitionOfDone `json:"definition_of_done"`
	Implementation   []Step           `json:"implementation"`
	Verification     Verification     `json:"verification"`
}

type DefinitionOfDone struct {
	Narrative    string   `json:"narrative"`
	Goals        []string `json:"goals"`
	CurrentState string   `json:"current_state"`
	ModuleShape  string   `json:"module_shape"`
}

type Step struct {
	Title       string       `json:"title"`
	Summary     string       `json:"summary"`
	FileChanges []FileChange `json:"file_changes"`
}

type FileChange struct {
	Filename    string `json:"filename"`
	Explanation string `json:"explanation"`
	Language    string `json:"language"`
	Code        string `json:"code"`
}

type Verification struct {
	Summary   string   `json:"summary"`
	Automated []string `json:"automated"`
	Manual    []string `json:"manual"`
}

func validatePlan(plan Plan) {
	if strings.TrimSpace(plan.Title) == "" || strings.TrimSpace(plan.Overview) == "" {
		panic("plan title and overview are required")
	}

	if strings.TrimSpace(plan.DefinitionOfDone.Narrative) == "" {
		panic("definition_of_done.narrative is required")
	}

	if len(plan.DefinitionOfDone.Goals) == 0 {
		panic("at least one definition_of_done goal is required")
	}

	if strings.TrimSpace(plan.DefinitionOfDone.CurrentState) == "" {
		panic("definition_of_done.current_state is required")
	}

	if strings.TrimSpace(plan.DefinitionOfDone.ModuleShape) == "" {
		panic("definition_of_done.module_shape is required")
	}

	if len(plan.Implementation) == 0 {
		panic("at least one implementation step is required")
	}

	for _, step := range plan.Implementation {
		if strings.TrimSpace(step.Title) == "" {
			panic("each implementation step needs a title")
		}

		if strings.TrimSpace(step.Summary) == "" {
			panic("each implementation step needs a summary")
		}

		if len(step.FileChanges) == 0 {
			panic("each implementation step needs file changes")
		}

		for _, change := range step.FileChanges {
			if strings.TrimSpace(change.Filename) == "" {
				panic("each file change needs a filename")
			}
			if strings.TrimSpace(change.Explanation) == "" {
				panic("each file change needs an explanation")
			}
			if strings.TrimSpace(change.Language) == "" {
				panic("each file change needs a language")
			}
			if strings.TrimSpace(change.Code) == "" {
				panic("each file change needs code")
			}
		}
	}
}
