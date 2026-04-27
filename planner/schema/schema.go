package schema

import (
	"bytes"
	"encoding/json"
	"fmt"
	"regexp"
	"strconv"
	"strings"
	"unicode"

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

// BuildPlanTemplate returns the canonical AI-authored plan skeleton.
func BuildPlanTemplate() Plan {
	return Plan{
		Title:    "<short title -- required, non-empty, max 88 chars>",
		Overview: "<2-4 sentence summary -- required, non-empty, max 500 chars>",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "<paragraph -- required, non-empty, max 500 chars>",
			Goals:        []ChecklistItem{{Text: "<concrete goal -- 1 to 8 items, each <= 88 chars>"}},
			CurrentState: "<current behavior with file:line refs -- required, non-empty, max 500 chars>",
			ModuleShape:  "<final layout -- required, non-empty, each line <= 88 chars>",
		},
		Implementation: []Step{
			{
				Title:   "<step title -- required, max 88 chars>",
				Summary: "<what changes and why -- required, max 500 chars>",
				FileChanges: []FileChange{
					{
						Filename:    "path/to/file",
						Explanation: "<one sentence, max 250 chars>",
						Diff:        "PLACEHOLDER",
					},
				},
			},
		},
		Verification: &Verification{
			Summary:   "<optional summary>",
			Automated: []ChecklistItem{{Text: "<runnable check, max 88 chars>"}},
			Manual:    []ChecklistItem{{Text: "<manual step, max 88 chars>"}},
		},
	}
}

// MarshalSection returns the JSON shape accepted by replace for the requested
// section, subsection, or field-level selector.
func MarshalSection(plan Plan, section, subsection, file, field string) ([]byte, error) {
	if field != "" {
		if section != "implementation" {
			return nil, fmt.Errorf("--field requires --section implementation")
		}
		if subsection == "" {
			return nil, fmt.Errorf("--field requires --subsection N")
		}
		idx, err := strconv.Atoi(subsection)
		if err != nil {
			return nil, fmt.Errorf("--subsection for implementation must be a 1-based integer index, got %q", subsection)
		}
		if idx < 1 || idx > len(plan.Implementation) {
			return nil, fmt.Errorf("implementation subsection %d out of range (have %d steps)", idx, len(plan.Implementation))
		}
		step := plan.Implementation[idx-1]
		switch field {
		case "title":
			return MarshalJSONNoEscape(step.Title)
		case "summary":
			return MarshalJSONNoEscape(step.Summary)
		case "filename":
			return MarshalJSONNoEscape("<filename>")
		case "explanation":
			return MarshalJSONNoEscape("<explanation>")
		default:
			return nil, fmt.Errorf("unknown --field %q", field)
		}
	}

	var value any
	switch section {
	case "title":
		if subsection != "" || file != "" {
			return nil, fmt.Errorf("--section title accepts no other selectors")
		}
		value = plan.Title
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
		switch subsection {
		case "":
			value = plan.Verification
		case "summary":
			value = plan.Verification.Summary
		case "automated":
			value = plan.Verification.Automated
		case "manual":
			value = plan.Verification.Manual
		default:
			return nil, fmt.Errorf("invalid verification subsection %q: valid values are summary, automated, manual", subsection)
		}
	default:
		return nil, fmt.Errorf("unknown section %q (valid: title, overview, definition_of_done, implementation, verification)", section)
	}

	raw, err := MarshalJSONNoEscape(value)
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
		"each file change filename must be non-empty, whitespace-free, at most 200 bytes, and path-shaped",
		"each file change must include a filename, explanation, and diff",
		"verification must be present",
		"title must be at most 88 characters",
		"overview must be at most 500 characters",
		"definition_of_done.narrative must be at most 500 characters",
		"definition_of_done.current_state must be at most 500 characters",
		"each line of definition_of_done.module_shape must be at most 88 characters",
		"each implementation step title must be at most 88 characters",
		"each implementation step summary must be at most 500 characters",
		"each file change explanation must be at most 250 characters",
		"each verification.automated[i].text must be at most 88 characters",
		"each verification.manual[i].text must be at most 88 characters",
	}
}
