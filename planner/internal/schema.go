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
// other value is rejected at decode and by ValidatePlan. "pending" is accepted
// only at the JSON boundary and normalized to empty.
type ChecklistStatus string

const (
	StatusDone ChecklistStatus = "done"
)

// ChecklistItem is one entry in a rendered checklist (goal or verification
// step). A plain-string JSON value decodes as ChecklistItem{Text: s} so
// existing callers keep working; object form must use {text, status}. The
// runtime state is empty or done; pending is accepted only as a decode-time
// alias.
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

func DecodePlan(data []byte) (Plan, error) {
	dec := json.NewDecoder(bytes.NewReader(data))
	dec.DisallowUnknownFields()
	var plan Plan
	if err := dec.Decode(&plan); err != nil {
		return Plan{}, fmt.Errorf("%s: %w", Lint(data, err), err)
	}
	if dec.More() {
		return Plan{}, fmt.Errorf("trailing data after plan JSON")
	}
	return plan, nil
}

// UnmarshalJSON accepts either a plain string (legacy payloads) or a strict
// {"text":...,"status":...} object. Plain strings decode with empty Status,
// rendered unchecked. Object form uses the shared strict decoder so typos in
// field names (e.g. "stats" instead of "status") fail loudly rather than
// silently round-tripping as unchecked.
func (c *ChecklistItem) UnmarshalJSON(data []byte) error {
	if len(data) > 0 && data[0] == '"' {
		var s string
		if err := json.Unmarshal(data, &s); err != nil {
			return err
		}
		c.Text = s
		c.Status = ""
		return nil
	}
	type raw ChecklistItem
	var r raw
	if err := DecodeStrict(data, &r); err != nil {
		return err
	}
	switch r.Status {
	case "", "pending":
		*c = ChecklistItem(r)
		c.Status = ""
		return nil
	case StatusDone:
		*c = ChecklistItem(r)
		return nil
	default:
		return fmt.Errorf("invalid checklist item status %q: want pending or done", r.Status)
	}
}

// MarshalJSONNoEscape renders JSON with indentation while preserving literal
// angle brackets in placeholder text. The planner template help surfaces use
// human-facing placeholders, so escaping them to \u003c and \u003e only makes
// the output harder to read.
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

func BuildPlanExample() Plan {
	return Plan{
		Title:    "Short title for the plan",
		Overview: "2-4 sentence summary of what the plan changes and why.",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Paragraph describing why the change matters and how the pieces fit together.",
			Goals:        []ChecklistItem{{Text: "Concrete acceptance criterion"}},
			CurrentState: "Current behavior, constraints, and relevant file:line references.",
			ModuleShape:  "Target file and directory structure after the change.",
		},
		Implementation: []Step{
			{
				Title:   "Short step title",
				Summary: "Narrative summary explaining what this step changes and why.",
				FileChanges: []FileChange{
					{
						Filename:    "path/to/file.ext",
						Explanation: "One sentence explaining why this code exists.",
						Diff:        "Unified diff of the change to this file, with context lines.",
					},
				},
			},
		},
		Verification: &Verification{
			Summary:   "Optional summary describing how verification maps to the goals.",
			Automated: []ChecklistItem{{Text: "Runnable automated check"}},
			Manual:    []ChecklistItem{{Text: "Manual verification step"}},
		},
	}
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
		"definition_of_done checklist items must have non-empty text; object status may be pending or done",
		"verification checklist items must have non-empty text; object status may be pending or done",
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
