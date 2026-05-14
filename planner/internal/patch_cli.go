package internal

import (
	"fmt"
	"io"
	"os"
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

	parsed, err := ParseMarkdown(string(sourceRaw))
	if err != nil {
		reportError(stderr, "patch", plannerMarkdownDecodeError(sourceRaw, err))
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

	plan := parsed.Plan
	if err := applyPlannerPatch(&plan, ops); err != nil {
		cliErr := mapReplaceCLIError(err, sourcePath)
		reportError(stderr, "patch", cliErr)
		return plannerExitCode(cliErr)
	}
	if err := ValidatePlan(plan); err != nil {
		reportError(stderr, "patch", newPlannerCLIError(PlannerValidateInputError, err, "updated plan"))
		return 1
	}

	rendered, err := RenderPlan(plan)
	if err != nil {
		reportError(stderr, "patch", newPlannerCLIError(PlannerRenderOutputError, err, "updated plan markdown"))
		return 1
	}
	frontmatter, _, err := splitFrontmatter(string(sourceRaw))
	if err != nil {
		reportError(stderr, "patch", plannerMarkdownDecodeError(sourceRaw, err))
		return 1
	}
	if err := WriteAtomic(outputPath, []byte(frontmatter+rendered)); err != nil {
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

func applyPlannerPatch(plan *Plan, ops []patchOp) error {
	for _, op := range ops {
		switch op.Kind {
		case patchOpUpdateField:
			if err := applyPlannerPatchField(plan, op.Selector, op.OldText, op.NewText); err != nil {
				return err
			}
		case patchOpAddItem:
			if err := applyPlannerPatchChecklistItem(plan, op.Selector, op.NewText); err != nil {
				return err
			}
		default:
			return fmt.Errorf("unknown patch operation")
		}
	}
	return nil
}

func applyPlannerPatchField(plan *Plan, selector, oldText, newText string) error {
	switch selector {
	case "title":
		if plan.Title != oldText {
			return newReplaceError(ReplacePatchMismatchError, fmt.Errorf("patch old value mismatch for title"))
		}
		plan.Title = newText
	case "overview":
		if plan.Overview != oldText {
			return newReplaceError(ReplacePatchMismatchError, fmt.Errorf("patch old value mismatch for overview"))
		}
		plan.Overview = newText
	case "definition_of_done.narrative":
		if plan.DefinitionOfDone.Narrative != oldText {
			return newReplaceError(ReplacePatchMismatchError, fmt.Errorf("patch old value mismatch for definition_of_done.narrative"))
		}
		plan.DefinitionOfDone.Narrative = newText
	case "definition_of_done.current_state":
		if plan.DefinitionOfDone.CurrentState != oldText {
			return newReplaceError(ReplacePatchMismatchError, fmt.Errorf("patch old value mismatch for definition_of_done.current_state"))
		}
		plan.DefinitionOfDone.CurrentState = newText
	case "definition_of_done.module_shape":
		if plan.DefinitionOfDone.ModuleShape != oldText {
			return newReplaceError(ReplacePatchMismatchError, fmt.Errorf("patch old value mismatch for definition_of_done.module_shape"))
		}
		plan.DefinitionOfDone.ModuleShape = newText
	case "verification.summary":
		if plan.Verification == nil {
			plan.Verification = &Verification{}
		}
		if plan.Verification.Summary != oldText {
			return newReplaceError(ReplacePatchMismatchError, fmt.Errorf("patch old value mismatch for verification.summary"))
		}
		plan.Verification.Summary = newText
	default:
		return newReplaceError(ReplacePatchSelectorError, fmt.Errorf("unsupported patch selector %q", selector))
	}
	return nil
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
