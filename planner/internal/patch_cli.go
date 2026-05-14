package internal

import (
	"fmt"
	"io"
	"os"
	"regexp"
	"strconv"
	"strings"
)

// Supported structured patch selectors.
//
// Scalar fields:
//   - title
//   - overview
//   - definition_of_done.narrative
//   - definition_of_done.current_state
//   - definition_of_done.module_shape
//   - verification.summary
//
// Checklist selectors (Add Item only):
//   - definition_of_done.goals
//   - verification.automated
//   - verification.manual
const patchUsage = "usage: planner patch <plan.md> [<out.md>]"

var (
	patchStepFieldSelectorRE       = regexp.MustCompile(`^implementation\[(-?\d+)\]\.(title|summary)$`)
	patchFileChangeFieldSelectorRE = regexp.MustCompile(`^implementation\[(-?\d+)\]\.file_changes\[(-?\d+)\]\.(filename|explanation)$`)
)

type patchFieldSelectorKind int

const (
	patchFieldSelectorTopLevel patchFieldSelectorKind = iota + 1
	patchFieldSelectorStep
	patchFieldSelectorFileChange
)

type patchFieldSelector struct {
	Kind            patchFieldSelectorKind
	TopLevel        string
	StepIndex       int
	FileChangeIndex int
	Field           string
}

type patchOpKind int

const (
	patchOpUpdateField patchOpKind = iota + 1
	patchOpAddItem
)

type patchOp struct {
	Kind     patchOpKind
	Selector string
	OldText  string
	NewText  string
}

func runPatch(args []string, stdout, stderr io.Writer) int {
	if len(args) < 1 || len(args) > 2 {
		reportError(stderr, "patch", newPlannerCLIError(PlannerUsageError, nil, patchUsage))
		return 2
	}
	// Execute strips global flags such as --json-errors before subcommand
	// dispatch. planner patch itself accepts no subcommand flags.
	for _, arg := range args {
		if strings.HasPrefix(arg, "--") {
			reportError(stderr, "patch", newPlannerCLIError(PlannerUsageError, nil, patchUsage))
			return 2
		}
	}

	sourcePath := args[0]
	outputPath := sourcePath
	if len(args) == 2 {
		outputPath = args[1]
	}

	sourceRaw, err := os.ReadFile(sourcePath)
	if err != nil {
		reportError(stderr, "patch", newPlannerCLIError(PlannerReadInputError, err, sourcePath))
		return 1
	}

	patchRaw, err := io.ReadAll(os.Stdin)
	if err != nil {
		reportError(stderr, "patch", newPlannerCLIError(PlannerReadInputError, err, "stdin"))
		return 1
	}

	ops, err := parsePlannerPatch(string(patchRaw))
	if err != nil {
		cliErr := mapReplaceCLIError(err, sourcePath)
		reportError(stderr, "patch", cliErr)
		return plannerExitCode(cliErr)
	}

	updatedRaw, err := applyPlannerPatch(sourceRaw, ops)
	if err != nil {
		cliErr := mapReplaceCLIError(err, sourcePath)
		reportError(stderr, "patch", cliErr)
		return plannerExitCode(cliErr)
	}
	if err := WriteAtomic(outputPath, updatedRaw); err != nil {
		reportError(stderr, "patch", newPlannerCLIError(PlannerWriteOutputError, err, outputPath))
		return 1
	}
	return 0
}

func parsePlannerPatch(raw string) ([]patchOp, error) {
	raw = strings.ReplaceAll(raw, "\r\n", "\n")
	lines := strings.Split(raw, "\n")
	if len(lines) == 0 || strings.TrimSpace(lines[0]) != "*** Begin Patch" {
		return nil, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("patch must start with *** Begin Patch"))
	}

	ops := []patchOp{}
	for i := 1; i < len(lines); {
		line := lines[i]
		if strings.TrimSpace(line) == "*** End Patch" {
			for _, rest := range lines[i+1:] {
				if strings.TrimSpace(rest) != "" {
					return nil, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("unexpected content after *** End Patch"))
				}
			}
			if len(ops) == 0 {
				return nil, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("patch must include at least one operation"))
			}
			return ops, nil
		}
		if line == "" {
			return nil, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("expected patch header, got empty line"))
		}
		if !strings.HasPrefix(line, "*** ") {
			return nil, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("expected patch header, got %q", line))
		}

		op, next, err := parsePlannerPatchOp(lines, i)
		if err != nil {
			return nil, err
		}
		ops = append(ops, op)
		i = next
	}

	return nil, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("missing *** End Patch"))
}

func parsePlannerPatchOp(lines []string, start int) (patchOp, int, error) {
	header := strings.TrimSpace(strings.TrimPrefix(lines[start], "*** "))
	verb, selector, ok := strings.Cut(header, ":")
	if !ok {
		return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("malformed patch header %q", lines[start]))
	}
	verb = strings.TrimSpace(verb)
	selector = strings.TrimSpace(selector)
	if selector == "" {
		return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("malformed patch header %q", lines[start]))
	}

	op := patchOp{Selector: selector}
	switch verb {
	case "Update Field":
		op.Kind = patchOpUpdateField
	case "Add Item":
		op.Kind = patchOpAddItem
	default:
		return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("unknown patch header %q", lines[start]))
	}

	bodyLines := []string{}
	next := len(lines)
	for i := start + 1; i < len(lines); i++ {
		line := lines[i]
		if strings.HasPrefix(line, "*** ") {
			next = i
			break
		}
		bodyLines = append(bodyLines, line)
	}

	switch op.Kind {
	case patchOpUpdateField:
		var oldLines, newLines []string
		minusPhase := true
		for _, line := range bodyLines {
			if line == "" {
				return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("update field bodies must prefix every line with + or -"))
			}
			switch line[0] {
			case '-':
				if !minusPhase {
					return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("update field bodies must list - lines before + lines"))
				}
				oldLines = append(oldLines, line[1:])
			case '+':
				minusPhase = false
				newLines = append(newLines, line[1:])
			default:
				return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("update field bodies must prefix every line with + or -"))
			}
		}
		if len(oldLines) == 0 || len(newLines) == 0 {
			return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("update field bodies require both - and + lines"))
		}
		op.OldText = strings.Join(oldLines, "\n")
		op.NewText = strings.Join(newLines, "\n")
	case patchOpAddItem:
		if len(bodyLines) != 1 {
			return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("checklist edits must be single-line"))
		}
		line := bodyLines[0]
		if line == "" || line[0] != '+' {
			return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("checklist edits must be single-line"))
		}
		op.NewText = line[1:]
	}

	return op, next, nil
}

func applyPlannerPatch(sourceRaw []byte, ops []patchOp) ([]byte, error) {
	currentRaw := sourceRaw
	for _, op := range ops {
		parsed, err := ParseMarkdown(string(currentRaw))
		if err != nil {
			return nil, newReplaceError(ReplaceParseSourceError, err)
		}
		switch op.Kind {
		case patchOpUpdateField:
			nextRaw, err := applyPlannerPatchField(currentRaw, parsed, op.Selector, op.OldText, op.NewText)
			if err != nil {
				return nil, err
			}
			currentRaw = nextRaw
		case patchOpAddItem:
			plan := parsed.Plan
			if err := applyPlannerPatchChecklistItem(&plan, op.Selector, op.NewText); err != nil {
				return nil, err
			}
			nextRaw, err := renderPatchedPlan(currentRaw, plan)
			if err != nil {
				return nil, err
			}
			currentRaw = nextRaw
		default:
			return nil, fmt.Errorf("unknown patch operation")
		}
	}
	return currentRaw, nil
}

func renderPatchedPlan(sourceRaw []byte, plan Plan) ([]byte, error) {
	if err := ValidatePlan(plan); err != nil {
		return nil, newReplaceError(ReplaceValidateResultError, err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		return nil, newReplaceError(ReplaceRenderResultError, err)
	}
	frontmatter, _, err := splitFrontmatter(string(sourceRaw))
	if err != nil {
		return nil, newReplaceError(ReplaceParseSourceError, err)
	}
	return []byte(frontmatter + rendered), nil
}

func parsePatchFieldSelector(raw string) (patchFieldSelector, error) {
	switch raw {
	case "title", "overview", "definition_of_done.narrative", "definition_of_done.current_state", "definition_of_done.module_shape", "verification.summary":
		return patchFieldSelector{Kind: patchFieldSelectorTopLevel, TopLevel: raw}, nil
	}
	if match := patchStepFieldSelectorRE.FindStringSubmatch(raw); match != nil {
		stepIdx, err := parsePatchSelectorIndex(match[1], raw, "step")
		if err != nil {
			return patchFieldSelector{}, err
		}
		return patchFieldSelector{Kind: patchFieldSelectorStep, StepIndex: stepIdx, Field: match[2]}, nil
	}
	if match := patchFileChangeFieldSelectorRE.FindStringSubmatch(raw); match != nil {
		stepIdx, err := parsePatchSelectorIndex(match[1], raw, "step")
		if err != nil {
			return patchFieldSelector{}, err
		}
		changeIdx, err := parsePatchSelectorIndex(match[2], raw, "file change")
		if err != nil {
			return patchFieldSelector{}, err
		}
		return patchFieldSelector{Kind: patchFieldSelectorFileChange, StepIndex: stepIdx, FileChangeIndex: changeIdx, Field: match[3]}, nil
	}
	return patchFieldSelector{}, newReplaceError(ReplacePatchSelectorError, fmt.Errorf("unsupported patch selector %q", raw))
}

func parsePatchSelectorIndex(raw, selector, segment string) (int, error) {
	idx, err := strconv.Atoi(raw)
	if err != nil {
		return 0, newReplaceError(ReplacePatchSelectorError, fmt.Errorf("unsupported patch selector %q", selector))
	}
	if idx < 1 {
		return 0, patchSelectorRangeError(selector, segment, idx, 0)
	}
	return idx, nil
}

func patchSelectorRangeError(selector, segment string, idx, have int) error {
	return newReplaceError(ReplacePatchSelectorError, fmt.Errorf("patch selector %q %s %d out of range (have %d)", selector, segment, idx, have))
}

func patchOldValueMismatch(selector string) error {
	return newReplaceError(ReplacePatchMismatchError, fmt.Errorf("patch old value mismatch for %s", selector))
}

func patchSelectorValue(plan Plan, selector patchFieldSelector) (string, error) {
	switch selector.Kind {
	case patchFieldSelectorTopLevel:
		switch selector.TopLevel {
		case "title":
			return plan.Title, nil
		case "overview":
			return plan.Overview, nil
		case "definition_of_done.narrative":
			return plan.DefinitionOfDone.Narrative, nil
		case "definition_of_done.current_state":
			return plan.DefinitionOfDone.CurrentState, nil
		case "definition_of_done.module_shape":
			return plan.DefinitionOfDone.ModuleShape, nil
		case "verification.summary":
			if plan.Verification == nil {
				return "", nil
			}
			return plan.Verification.Summary, nil
		}
	case patchFieldSelectorStep:
		if selector.StepIndex > len(plan.Implementation) {
			return "", patchSelectorRangeError(fmt.Sprintf("implementation[%d].%s", selector.StepIndex, selector.Field), "step", selector.StepIndex, len(plan.Implementation))
		}
		step := plan.Implementation[selector.StepIndex-1]
		if selector.Field == "title" {
			return step.Title, nil
		}
		return step.Summary, nil
	case patchFieldSelectorFileChange:
		if selector.StepIndex > len(plan.Implementation) {
			return "", patchSelectorRangeError(fmt.Sprintf("implementation[%d].file_changes[%d].%s", selector.StepIndex, selector.FileChangeIndex, selector.Field), "step", selector.StepIndex, len(plan.Implementation))
		}
		step := plan.Implementation[selector.StepIndex-1]
		if selector.FileChangeIndex > len(step.FileChanges) {
			return "", patchSelectorRangeError(fmt.Sprintf("implementation[%d].file_changes[%d].%s", selector.StepIndex, selector.FileChangeIndex, selector.Field), "file change", selector.FileChangeIndex, len(step.FileChanges))
		}
		change := step.FileChanges[selector.FileChangeIndex-1]
		if selector.Field == "filename" {
			return change.Filename, nil
		}
		return change.Explanation, nil
	}
	return "", newReplaceError(ReplacePatchSelectorError, fmt.Errorf("unsupported patch selector"))
}

func applyPlannerPatchField(sourceRaw []byte, parsed ParseResult, selector, oldText, newText string) ([]byte, error) {
	field, err := parsePatchFieldSelector(selector)
	if err != nil {
		return nil, err
	}
	current, err := patchSelectorValue(parsed.Plan, field)
	if err != nil {
		return nil, err
	}
	if current != oldText {
		return nil, patchOldValueMismatch(selector)
	}
	switch field.Kind {
	case patchFieldSelectorTopLevel:
		plan := parsed.Plan
		switch field.TopLevel {
		case "title":
			plan.Title = newText
		case "overview":
			plan.Overview = newText
		case "definition_of_done.narrative":
			plan.DefinitionOfDone.Narrative = newText
		case "definition_of_done.current_state":
			plan.DefinitionOfDone.CurrentState = newText
		case "definition_of_done.module_shape":
			plan.DefinitionOfDone.ModuleShape = newText
		case "verification.summary":
			if plan.Verification == nil {
				plan.Verification = &Verification{}
			}
			plan.Verification.Summary = newText
		}
		return renderPatchedPlan(sourceRaw, plan)
	case patchFieldSelectorStep, patchFieldSelectorFileChange:
		opts := ReplaceOptions{Section: "implementation", Subsection: strconv.Itoa(field.StepIndex), Field: field.Field, Raw: true}
		out, _, err := spliceImplementationScalarByIndex(string(sourceRaw), parsed.Plan, parsed.Steps, opts, field.StepIndex, field.FileChangeIndex, newText)
		if err != nil {
			return nil, err
		}
		return []byte(out), nil
	}
	return nil, newReplaceError(ReplacePatchSelectorError, fmt.Errorf("unsupported patch selector %q", selector))
}

func applyPlannerPatchChecklistItem(plan *Plan, selector, text string) error {
	item := ChecklistItem{Text: text}
	switch selector {
	case "definition_of_done.goals":
		plan.DefinitionOfDone.Goals = append(plan.DefinitionOfDone.Goals, item)
	case "verification.automated":
		if plan.Verification == nil {
			plan.Verification = &Verification{}
		}
		plan.Verification.Automated = append(plan.Verification.Automated, item)
	case "verification.manual":
		if plan.Verification == nil {
			plan.Verification = &Verification{}
		}
		plan.Verification.Manual = append(plan.Verification.Manual, item)
	default:
		return newReplaceError(ReplacePatchSelectorError, fmt.Errorf("unsupported checklist selector %q", selector))
	}
	return nil
}
