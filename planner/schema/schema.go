package schema

import (
	"bytes"
	"encoding/json"
	"fmt"
	"strconv"

	"planner/internal/jsoninput"
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
		return Plan{}, fmt.Errorf("%s: %w", jsoninput.Lint(data, err), err)
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
	if err := jsoninput.DecodeStrict(data, &r); err != nil {
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

// BuildPlanTemplate returns the canonical AI-authored plan skeleton.
func BuildPlanTemplate() Plan {
	return Plan{
		Title:    "<short title -- required, non-empty>",
		Overview: "<2-4 sentence summary -- required, non-empty>",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "<paragraph -- required, non-empty>",
			Goals:        []ChecklistItem{{Text: "<concrete goal -- 1 to 8 items, each <= 88 chars>"}},
			CurrentState: "<current behavior with file:line refs -- required, non-empty>",
			ModuleShape:  "<final layout -- required, non-empty>",
		},
		Implementation: []Step{
			{
				Title:   "<step title -- required>",
				Summary: "<what changes and why -- required>",
				FileChanges: []FileChange{
					{
						Filename:    "<path/to/file>",
						Explanation: "<one sentence>",
						Diff:        "PLACEHOLDER",
					},
				},
			},
		},
		Verification: &Verification{
			Summary:   "<optional summary>",
			Automated: []ChecklistItem{{Text: "<runnable check>"}},
			Manual:    []ChecklistItem{{Text: "<manual step>"}},
		},
	}
}

// MarshalSection returns the JSON shape accepted by replace for the requested
// section or subsection.
func MarshalSection(plan Plan, section, subsection string) ([]byte, error) {
	var value any
	switch section {
	case "overview":
		if subsection != "" {
			return nil, fmt.Errorf("overview does not support subsections")
		}
		value = plan.Overview
	case "definition_of_done":
		switch subsection {
		case "":
			value = plan.DefinitionOfDone
		case "narrative":
			value = plan.DefinitionOfDone.Narrative
		case "goals":
			value = plan.DefinitionOfDone.Goals
		case "current_state":
			value = plan.DefinitionOfDone.CurrentState
		case "module_shape":
			value = plan.DefinitionOfDone.ModuleShape
		default:
			return nil, fmt.Errorf("unknown definition_of_done subsection %q", subsection)
		}
	case "implementation":
		if subsection == "" {
			value = plan.Implementation
			break
		}
		idx, err := strconv.Atoi(subsection)
		if err != nil {
			return nil, fmt.Errorf("--subsection for implementation must be a 1-based integer index, got %q", subsection)
		}
		if idx < 1 || idx > len(plan.Implementation) {
			return nil, fmt.Errorf("implementation subsection %d out of range (have %d steps)", idx, len(plan.Implementation))
		}
		value = plan.Implementation[idx-1]
	case "verification":
		if subsection != "" {
			return nil, fmt.Errorf("verification subsections are deferred to PDEV-028; today only --section verification is supported")
		}
		value = plan.Verification
	default:
		return nil, fmt.Errorf("unknown section %q (valid: overview, definition_of_done, implementation, verification)", section)
	}

	raw, err := json.MarshalIndent(value, "", "  ")
	if err != nil {
		return nil, err
	}
	return append(raw, '\n'), nil
}

func ValidationRules() []string {
	return []string{
		"title, overview, definition_of_done.narrative, definition_of_done.current_state, and definition_of_done.module_shape must be non-empty",
		"definition_of_done.goals must contain between 1 and 8 items",
		"definition_of_done checklist items must have non-empty text; object status may be pending or done",
		"verification checklist items must have non-empty text; object status may be pending or done",
		"each definition_of_done.goals item must be at most 88 characters",
		"implementation must contain at least one step",
		"each implementation step must include a title, summary, and at least one file change",
		"each file change must include a filename, explanation, and diff",
		"verification must be present",
	}
}
