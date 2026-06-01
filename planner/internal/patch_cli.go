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
	patchFileChangeSelectorRE      = regexp.MustCompile(`^implementation\[(-?\d+)\]\.file_changes\[(-?\d+)\]$`)
	patchStepSelectorRE            = regexp.MustCompile(`^implementation\[(-?\d+)\]$`)
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
	patchOpUpdateDiff
	patchOpAddFileChange
	patchOpAddStep
	patchOpAddItem
)

type patchOp struct {
	Kind        patchOpKind
	Selector    string
	OldText     string
	NewText     string
	Expect      string
	File        string
	Explanation string
	Title       string
	Summary     string
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
	d := diffLines(string(sourceRaw), string(updatedRaw))
	if err := WriteAtomic(outputPath, updatedRaw); err != nil {
		reportError(stderr, "patch", newPlannerCLIError(PlannerWriteOutputError, err, outputPath))
		return 1
	}
	if d != "" {
		_, _ = io.WriteString(stdout, d)
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
		if op.Kind == patchOpUpdateDiff && len(ops) > 0 {
			return nil, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("update diff must be the only patch operation"))
		}
		ops = append(ops, op)
		i = next
	}

	if len(ops) == 1 && ops[0].Kind == patchOpUpdateDiff {
		return ops, nil
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
	case "Update Diff":
		op.Kind = patchOpUpdateDiff
	case "Add File Change":
		op.Kind = patchOpAddFileChange
	case "Add Step":
		op.Kind = patchOpAddStep
	case "Add Item":
		op.Kind = patchOpAddItem
	default:
		return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("unknown patch header %q", lines[start]))
	}

	if op.Kind == patchOpUpdateDiff || op.Kind == patchOpAddFileChange || op.Kind == patchOpAddStep {
		headersEnd := start + 1
		for headersEnd < len(lines) {
			line := lines[headersEnd]
			if !strings.HasPrefix(line, "*** ") || isPatchOperationHeader(line) {
				break
			}
			headersEnd++
		}
		headers, _, err := splitPatchHeaders(lines[start+1 : headersEnd])
		if err != nil {
			return patchOp{}, 0, err
		}
		bodyEnd := len(lines)
		for i := headersEnd; i < len(lines); i++ {
			line := lines[i]
			if strings.TrimSpace(line) == "*** End Patch" || isPatchOperationHeader(line) {
				bodyEnd = i
				break
			}
		}
		if op.Kind == patchOpUpdateDiff && bodyEnd < len(lines) && strings.TrimSpace(lines[bodyEnd]) != "*** End Patch" {
			return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("update diff must be the only patch operation"))
		}
		bodyLines := lines[headersEnd:bodyEnd]
		switch op.Kind {
		case patchOpUpdateDiff:
			op.Expect = headers["Expect"]
			op.File = headers["File"]
			if op.Expect == "" || op.File == "" {
				return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("update diff requires *** Expect and *** File headers"))
			}
			oldText, newText, err := parseSignedPatchBody(bodyLines, "update diff bodies")
			if err != nil {
				return patchOp{}, 0, err
			}
			op.OldText = oldText
			op.NewText = newText
		case patchOpAddFileChange:
			op.File = headers["File"]
			op.Explanation = headers["Explanation"]
			if op.File == "" || op.Explanation == "" {
				return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("add file change requires *** File and *** Explanation headers"))
			}
			oldText, newText, err := parseSignedPatchBody(bodyLines, "add file change bodies")
			if err != nil {
				return patchOp{}, 0, err
			}
			op.OldText = oldText
			op.NewText = newText
		case patchOpAddStep:
			op.Title = headers["Title"]
			op.Summary = headers["Summary"]
			op.File = headers["File"]
			op.Explanation = headers["Explanation"]
			if op.Title == "" || op.Summary == "" || op.File == "" || op.Explanation == "" {
				return patchOp{}, 0, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("add step requires *** Title, *** Summary, *** File, and *** Explanation headers"))
			}
			oldText, newText, err := parseSignedPatchBody(bodyLines, "add step bodies")
			if err != nil {
				return patchOp{}, 0, err
			}
			op.OldText = oldText
			op.NewText = newText
		}
		return op, bodyEnd, nil
	}

	bodyLines := []string{}
	next := len(lines)
	for i := start + 1; i < len(lines); i++ {
		line := lines[i]
		if strings.TrimSpace(line) == "*** End Patch" || isPatchOperationHeader(line) {
			next = i
			break
		}
		bodyLines = append(bodyLines, line)
	}

	switch op.Kind {
	case patchOpUpdateField:
		oldText, newText, err := parseSignedPatchBody(bodyLines, "update field bodies")
		if err != nil {
			return patchOp{}, 0, err
		}
		op.OldText = oldText
		op.NewText = newText
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
		case patchOpUpdateDiff:
			nextRaw, err := applyPlannerPatchGeneratedDiff(currentRaw, parsed, op)
			if err != nil {
				return nil, err
			}
			currentRaw = nextRaw
		case patchOpAddFileChange:
			nextRaw, err := applyPlannerPatchAddFileChange(currentRaw, parsed, op)
			if err != nil {
				return nil, err
			}
			currentRaw = nextRaw
		case patchOpAddStep:
			nextRaw, err := applyPlannerPatchAddStep(currentRaw, parsed, op)
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

func isPatchOperationHeader(line string) bool {
	trimmed := strings.TrimSpace(strings.TrimPrefix(line, "*** "))
	if trimmed == "End Patch" {
		return true
	}
	verb, _, ok := strings.Cut(trimmed, ":")
	if !ok {
		return false
	}
	switch strings.TrimSpace(verb) {
	case "Update Field", "Update Diff", "Add Item", "Add File Change", "Add Step":
		return true
	default:
		return false
	}
}

func splitPatchHeaders(lines []string) (map[string]string, []string, error) {
	headers := map[string]string{}
	idx := 0
	for idx < len(lines) {
		line := lines[idx]
		if !strings.HasPrefix(line, "*** ") {
			break
		}
		nameValue := strings.TrimSpace(strings.TrimPrefix(line, "*** "))
		name, value, ok := strings.Cut(nameValue, ":")
		if !ok {
			return nil, nil, newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("malformed patch header %q", line))
		}
		headers[strings.TrimSpace(name)] = strings.TrimSpace(value)
		idx++
	}
	return headers, lines[idx:], nil
}

func parseSignedPatchBody(lines []string, subject string) (string, string, error) {
	var oldLines, newLines []string
	minusPhase := true
	for _, line := range lines {
		if line == "" {
			return "", "", newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("%s must prefix every line with + or -", subject))
		}
		switch line[0] {
		case '-':
			if !minusPhase {
				return "", "", newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("%s must list - lines before + lines", subject))
			}
			oldLines = append(oldLines, line[1:])
		case '+':
			minusPhase = false
			newLines = append(newLines, line[1:])
		default:
			return "", "", newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("%s must prefix every line with + or -", subject))
		}
	}
	if len(newLines) == 0 {
		return "", "", newReplaceError(ReplacePatchSyntaxError, fmt.Errorf("%s require at least one + line", subject))
	}
	return strings.Join(oldLines, "\n"), strings.Join(newLines, "\n"), nil
}

func applyPlannerPatchGeneratedDiff(sourceRaw []byte, parsed ParseResult, op patchOp) ([]byte, error) {
	stepIdx, changeIdx, err := parsePatchFileChangeSelector(op.Selector)
	if err != nil {
		return nil, err
	}
	if stepIdx > len(parsed.Plan.Implementation) {
		return nil, patchSelectorRangeError(op.Selector, "step", stepIdx, len(parsed.Plan.Implementation))
	}
	step := parsed.Plan.Implementation[stepIdx-1]
	if changeIdx > len(step.FileChanges) {
		return nil, patchSelectorRangeError(op.Selector, "file change", changeIdx, len(step.FileChanges))
	}
	change := step.FileChanges[changeIdx-1]
	raw := rawAt(string(sourceRaw), parsed.DiffContents[stepIdx-1][changeIdx-1])
	currentExpect := buildUpdateDiffExpect(op.Selector, change.Filename, change.Explanation, raw)
	if op.Expect != currentExpect {
		return nil, newReplaceError(ReplacePatchExpectMismatchError, fmt.Errorf("patch diff token mismatch for %s", op.Selector))
	}
	if op.File != change.Filename {
		return nil, newReplaceError(ReplacePatchMismatchError, fmt.Errorf("patch filename mismatch for %s", op.Selector))
	}
	generated, err := generateUnifiedDiff(op.File, op.OldText, op.NewText)
	if err != nil {
		return nil, err
	}
	plan := parsed.Plan
	plan.Implementation[stepIdx-1].FileChanges[changeIdx-1].Diff = generated
	return renderPatchedPlan(sourceRaw, plan)
}

func applyPlannerPatchAddFileChange(sourceRaw []byte, parsed ParseResult, op patchOp) ([]byte, error) {
	stepMatch := patchStepSelectorRE.FindStringSubmatch(op.Selector)
	if stepMatch == nil {
		return nil, newReplaceError(ReplacePatchSelectorError, fmt.Errorf("unsupported patch selector %q", op.Selector))
	}
	stepIdx, err := parsePatchSelectorIndex(stepMatch[1], op.Selector, "step")
	if err != nil {
		return nil, err
	}
	if stepIdx > len(parsed.Plan.Implementation) {
		return nil, patchSelectorRangeError(op.Selector, "step", stepIdx, len(parsed.Plan.Implementation))
	}
	generated, err := generateUnifiedDiff(op.File, op.OldText, op.NewText)
	if err != nil {
		return nil, err
	}
	plan := parsed.Plan
	plan.Implementation[stepIdx-1].FileChanges = append(plan.Implementation[stepIdx-1].FileChanges, FileChange{Filename: op.File, Explanation: op.Explanation, Diff: generated})
	return renderPatchedPlan(sourceRaw, plan)
}

func applyPlannerPatchAddStep(sourceRaw []byte, parsed ParseResult, op patchOp) ([]byte, error) {
	if op.Selector != "implementation" {
		return nil, newReplaceError(ReplacePatchSelectorError, fmt.Errorf("unsupported patch selector %q", op.Selector))
	}
	generated, err := generateUnifiedDiff(op.File, op.OldText, op.NewText)
	if err != nil {
		return nil, err
	}
	plan := parsed.Plan
	plan.Implementation = append(plan.Implementation, Step{Title: op.Title, Summary: op.Summary, FileChanges: []FileChange{{Filename: op.File, Explanation: op.Explanation, Diff: generated}}})
	return renderPatchedPlan(sourceRaw, plan)
}

func parsePatchFileChangeSelector(selector string) (int, int, error) {
	match := patchFileChangeSelectorRE.FindStringSubmatch(selector)
	if match == nil {
		return 0, 0, newReplaceError(ReplacePatchSelectorError, fmt.Errorf("unsupported patch selector %q", selector))
	}
	stepIdx, err := parsePatchSelectorIndex(match[1], selector, "step")
	if err != nil {
		return 0, 0, err
	}
	changeIdx, err := parsePatchSelectorIndex(match[2], selector, "file change")
	if err != nil {
		return 0, 0, err
	}
	return stepIdx, changeIdx, nil
}

func applyPlannerPatchDiff(sourceRaw []byte, parsed ParseResult, selector, expect, newText string) ([]byte, error) {
	stepIdx, changeIdx, err := parsePatchFileChangeSelector(selector)
	if err != nil {
		return nil, err
	}
	if stepIdx > len(parsed.Plan.Implementation) {
		return nil, patchSelectorRangeError(selector, "step", stepIdx, len(parsed.Plan.Implementation))
	}
	step := parsed.Plan.Implementation[stepIdx-1]
	if changeIdx > len(step.FileChanges) {
		return nil, patchSelectorRangeError(selector, "file change", changeIdx, len(step.FileChanges))
	}
	change := step.FileChanges[changeIdx-1]
	raw := rawAt(string(sourceRaw), parsed.DiffContents[stepIdx-1][changeIdx-1])
	currentExpect := buildUpdateDiffExpect(selector, change.Filename, change.Explanation, raw)
	if expect != currentExpect {
		return nil, newReplaceError(ReplacePatchExpectMismatchError, fmt.Errorf("patch diff token mismatch for %s", selector))
	}
	opts := ReplaceOptions{Section: "implementation", Subsection: strconv.Itoa(stepIdx), Field: "diff"}
	out, _, err := spliceImplementationDiffByIndex(string(sourceRaw), parsed.Plan, parsed.DiffContents, opts, stepIdx, changeIdx, newText)
	if err != nil {
		return nil, err
	}
	return []byte(out), nil
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
