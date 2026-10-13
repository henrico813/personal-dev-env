package internal

import (
	"bytes"
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
	"unicode"
)

const (
	MaxTitleLength                 = 66
	MaxOverviewLength              = 250
	MaxDoDNarrativeLength          = 250
	MaxDoDGoals                    = 6
	MaxDoDGoalLength               = 66
	MaxCurrentStateLength          = 250
	MaxModuleShapeLineLength       = 66
	MaxStepTitleLength             = 66
	MaxStepSummaryLength           = 250
	MaxFileChangeExplanationLength = 175
	MaxVerificationItemTextLength  = 66
)

type Plan struct {
	Title            string           `json:"title"`
	Overview         string           `json:"overview"`
	DefinitionOfDone DefinitionOfDone `json:"definition_of_done"`
	Implementation   []Step           `json:"implementation"`
	Verification     *Verification    `json:"verification"`
}

// ChecklistStatus is a closed enum of completion states for a ChecklistItem.
// The empty string renders as "- [ ]"; StatusDone renders as "- [x]". Any
// other value is rejected by ValidatePlan.
type ChecklistStatus string

const (
	StatusDone ChecklistStatus = "done"
)

// ChecklistItem is one entry in a rendered checklist (goal or verification
// step).
type ChecklistItem struct {
	Text   string          `json:"text"`
	Status ChecklistStatus `json:"status,omitempty"`
}

// IsDone reports whether the item is in the done state.
func (c ChecklistItem) IsDone() bool { return c.Status == StatusDone }

type DefinitionOfDone struct {
	Narrative    string          `json:"narrative"`
	Goals        []ChecklistItem `json:"goals"`
	CurrentState string          `json:"current_state"`
	ModuleShape  string          `json:"module_shape"`
}

type Step struct {
	Title       string       `json:"title"`
	Summary     string       `json:"summary"`
	FileChanges []FileChange `json:"file_changes"`
}

type FileChange struct {
	Filename    string `json:"filename"`
	Explanation string `json:"explanation"`
	Diff        string `json:"diff"`
}

type Verification struct {
	Summary   string          `json:"summary"`
	Automated []ChecklistItem `json:"automated"`
	Manual    []ChecklistItem `json:"manual"`
}

// MarshalJSONNoEscape renders indented JSON without escaping <, >, and &, so
// plan titles and paths in planner list JSON stay readable.
func MarshalJSONNoEscape(value any) ([]byte, error) {
	var buf bytes.Buffer
	enc := json.NewEncoder(&buf)
	enc.SetEscapeHTML(false)
	enc.SetIndent("", "  ")
	if err := enc.Encode(value); err != nil {
		return nil, err
	}
	return bytes.TrimSuffix(buf.Bytes(), []byte{'\n'}), nil
}

var filenameShape = regexp.MustCompile(`^[A-Za-z0-9._-]+(?:/[A-Za-z0-9._-]+)*$`)

const MaxFilenameLength = 200

// ValidateFilenameShape is the shared filename rule for rendered plans.
func ValidateFilenameShape(name string) error {
	trimmed := strings.TrimSpace(name)
	if trimmed == "" {
		return fmt.Errorf("invalid file change filename %q: empty after trim", name)
	}
	if strings.IndexFunc(name, unicode.IsSpace) >= 0 {
		return fmt.Errorf("invalid file change filename %q: contains whitespace", name)
	}
	if len(trimmed) > MaxFilenameLength {
		return fmt.Errorf("invalid file change filename %q: %d bytes exceeds %d-byte limit", name, len(trimmed), MaxFilenameLength)
	}
	if !filenameShape.MatchString(trimmed) {
		return fmt.Errorf("invalid file change filename %q: not a path-shape (expected components matching [A-Za-z0-9._-]+ joined by /)", name)
	}
	return nil
}

// BuildPlanTemplate returns the AI-authored plan skeleton.
func BuildPlanTemplate() Plan {
	return Plan{
		Title:    fmt.Sprintf("<short title -- required, non-empty, max %d chars>", MaxTitleLength),
		Overview: fmt.Sprintf("<2-4 sentence summary -- required, non-empty, max %d chars>", MaxOverviewLength),
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    fmt.Sprintf("<paragraph -- required, non-empty, max %d chars>", MaxDoDNarrativeLength),
			Goals:        []ChecklistItem{{Text: fmt.Sprintf("<concrete goal -- 1 to %d items, each <= %d chars>", MaxDoDGoals, MaxDoDGoalLength)}},
			CurrentState: fmt.Sprintf("<current behavior with file:line refs -- required, non-empty, max %d chars>", MaxCurrentStateLength),
			ModuleShape:  fmt.Sprintf("<final layout -- required, non-empty, each line <= %d chars>", MaxModuleShapeLineLength),
		},
		Implementation: []Step{
			{
				Title:   fmt.Sprintf("<step title -- required, max %d chars>", MaxStepTitleLength),
				Summary: fmt.Sprintf("<what changes and why -- required, max %d chars>", MaxStepSummaryLength),
				FileChanges: []FileChange{
					{
						Filename:    "path/to/file",
						Explanation: fmt.Sprintf("<one sentence, max %d chars>", MaxFileChangeExplanationLength),
						Diff:        "PLACEHOLDER",
					},
				},
			},
		},
		Verification: &Verification{
			Summary:   "<optional summary>",
			Automated: []ChecklistItem{{Text: fmt.Sprintf("<runnable check, max %d chars>", MaxVerificationItemTextLength)}},
			Manual:    []ChecklistItem{{Text: fmt.Sprintf("<manual step, max %d chars>", MaxVerificationItemTextLength)}},
		},
	}
}

func ValidationRules() []string {
	return []string{
		"title, overview, definition_of_done.narrative, definition_of_done.current_state, and definition_of_done.module_shape must be non-empty",
		fmt.Sprintf("definition_of_done.goals must contain between 1 and %d items", MaxDoDGoals),
		"definition_of_done checklist items must have non-empty text",
		"verification checklist items must have non-empty text",
		fmt.Sprintf("each definition_of_done.goals item must be at most %d characters", MaxDoDGoalLength),
		"implementation must contain at least one step",
		"each implementation step must include a title, summary, and at least one file change",
		"each file change filename must be unique per step, non-empty, whitespace-free, at most 200 bytes, and path-shaped",
		"each file change must include a filename, explanation, and diff",
		"verification must be present",
		fmt.Sprintf("title must be at most %d characters", MaxTitleLength),
		fmt.Sprintf("overview must be at most %d characters", MaxOverviewLength),
		fmt.Sprintf("definition_of_done.narrative must be at most %d characters", MaxDoDNarrativeLength),
		fmt.Sprintf("definition_of_done.current_state must be at most %d characters", MaxCurrentStateLength),
		fmt.Sprintf("each line of definition_of_done.module_shape must be at most %d characters", MaxModuleShapeLineLength),
		fmt.Sprintf("each implementation step title must be at most %d characters", MaxStepTitleLength),
		fmt.Sprintf("each implementation step summary must be at most %d characters", MaxStepSummaryLength),
		fmt.Sprintf("each file change explanation must be at most %d characters", MaxFileChangeExplanationLength),
		fmt.Sprintf("each verification.automated[i].text must be at most %d characters", MaxVerificationItemTextLength),
		fmt.Sprintf("each verification.manual[i].text must be at most %d characters", MaxVerificationItemTextLength),
	}
}
