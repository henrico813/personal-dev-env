// Package replace provides splice-based replacement for sections and subsections
// in canonical planner-rendered markdown while preserving non-targeted content
// byte-for-byte.
package replace

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"

	"planner/inspect"
	"planner/internal/jsoninput"
	"planner/render"
	"planner/schema"
	"planner/validate"
)

// ReplaceResult describes what was replaced in a successful Run call.
type ReplaceResult struct {
	Section       string `json:"section"`
	Subsection    string `json:"subsection,omitempty"`
	File          string `json:"file,omitempty"`
	Field         string `json:"field,omitempty"`
	StepsReplaced []int  `json:"steps_replaced,omitempty"`
	Appended      bool   `json:"appended,omitempty"`
}

// ReplaceOptions selects the target scope for a replace operation.
// Section is required; Subsection and Append are mutually exclusive; File and
// Field address a leaf selector inside implementation.
type ReplaceOptions struct {
	Section    string // required: overview|definition_of_done|implementation|verification
	Subsection string // field name for definition_of_done; 1-based step index for implementation
	Append     bool   // append a new step to implementation
	File       string // filename selector for implementation diff-field splices
	Field      string // field selector for implementation leaf updates
}

// Run replaces the targeted section or subsection in sourcePath with the patch
// from patchPath, writing the result atomically to outputPath.
func Run(sourcePath string, opts ReplaceOptions, patchPath string, outputPath string) (ReplaceResult, error) {
	patchRaw, err := os.ReadFile(patchPath)
	if err != nil {
		return ReplaceResult{}, err
	}
	return RunFromData(sourcePath, opts, patchRaw, outputPath)
}

// RunFromData is Run but with pre-read patch bytes, so the CLI can stream the
// patch from stdin without staging a temp file. Behavior is otherwise
// identical: source read from sourcePath, output written atomically to
// outputPath, non-targeted sections preserved byte-for-byte.
func RunFromData(sourcePath string, opts ReplaceOptions, patchRaw []byte, outputPath string) (ReplaceResult, error) {
	out, result, err := PreviewFromData(sourcePath, opts, patchRaw)
	if err != nil {
		return ReplaceResult{}, err
	}
	if err := WriteAtomic(outputPath, []byte(out)); err != nil {
		return ReplaceResult{}, err
	}
	return result, nil
}

// PreviewFromData runs the replace logic in memory and returns the post-splice
// string plus the ReplaceResult. It performs no writes, so --diff can render an
// accurate preview with no filesystem side effects. All existing error shapes
// (invalid section, parse, validation, preservation) surface here; writes no
// longer gate them.
func PreviewFromData(sourcePath string, opts ReplaceOptions, patchRaw []byte) (string, ReplaceResult, error) {
	if err := validateOpts(opts); err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceInvalidOptionsError, err)
	}

	sourceRaw, err := os.ReadFile(sourcePath)
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceReadSourceError, err)
	}

	plan, sectionSpans, stepSpans, diffSpans, err := inspect.ParseMarkdown(string(sourceRaw))
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceParseSourceError, err)
	}

	if opts.Field != "" {
		return applyFieldPatch(string(sourceRaw), opts, patchRaw, plan, diffSpans)
	}

	updated := plan
	stepsReplaced := []int{}
	appended := false
	switch opts.Section {
	case "overview":
		var text string
		if err := decodePatch(patchRaw, &text); err != nil {
			return "", ReplaceResult{}, err
		}
		updated.Overview = text
	case "definition_of_done":
		if err := applyDoDPatch(&updated, opts.Subsection, patchRaw); err != nil {
			return "", ReplaceResult{}, err
		}
	case "implementation":
		var err error
		stepsReplaced, appended, err = applyImplementationPatch(&updated, opts, patchRaw)
		if err != nil {
			return "", ReplaceResult{}, err
		}
	case "verification":
		var v schema.Verification
		if err := decodePatch(patchRaw, &v); err != nil {
			return "", ReplaceResult{}, err
		}
		updated.Verification = &v
	}

	if err := validate.ValidatePlan(updated); err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceValidateResultError, err)
	}

	out, err := applySplice(string(sourceRaw), updated, opts, sectionSpans, stepSpans)
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceRenderResultError, err)
	}

	_, newSectionSpans, newStepSpans, _, err := inspect.ParseMarkdown(out)
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceRenderResultError, err)
	}

	if err := assertPreserved(string(sourceRaw), out, opts, sectionSpans, newSectionSpans, stepSpans, newStepSpans); err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceValidateResultError, err)
	}

	return out, ReplaceResult{
		Section:       opts.Section,
		Subsection:    opts.Subsection,
		StepsReplaced: stepsReplaced,
		Appended:      appended,
	}, nil
}

func validateOpts(opts ReplaceOptions) error {
	switch opts.Section {
	case "overview", "definition_of_done", "implementation", "verification":
	default:
		return fmt.Errorf("invalid section %q: valid values are overview, definition_of_done, implementation, verification", opts.Section)
	}
	if opts.Append && opts.Section != "implementation" {
		return fmt.Errorf("--append is only valid with --section implementation")
	}
	if opts.Append && opts.Subsection != "" {
		return fmt.Errorf("--append and --subsection cannot be used together")
	}
	if opts.Append && opts.Field != "" {
		return fmt.Errorf("--append cannot be used with --field")
	}
	if (opts.Section == "overview" || opts.Section == "verification") && opts.Subsection != "" {
		return fmt.Errorf("--subsection is not supported for section %q", opts.Section)
	}
	if opts.Field != "" {
		if opts.Section != "implementation" {
			return fmt.Errorf("--field requires --section implementation")
		}
		if opts.Subsection == "" {
			return fmt.Errorf("--field requires --subsection N")
		}
		if opts.File == "" {
			return fmt.Errorf("--field requires --file F")
		}
	}
	if opts.File != "" && opts.Field == "" {
		return fmt.Errorf("--file requires --field")
	}
	return nil
}

func applyDoDPatch(plan *schema.Plan, subsection string, patchRaw []byte) error {
	if subsection == "" {
		var dod schema.DefinitionOfDone
		if err := decodePatch(patchRaw, &dod); err != nil {
			return err
		}
		plan.DefinitionOfDone = dod
		return nil
	}
	switch subsection {
	case "narrative":
		var s string
		if err := decodePatch(patchRaw, &s); err != nil {
			return err
		}
		plan.DefinitionOfDone.Narrative = s
	case "goals":
		var goals []schema.ChecklistItem
		if err := decodePatch(patchRaw, &goals); err != nil {
			return err
		}
		plan.DefinitionOfDone.Goals = goals
	case "current_state":
		var s string
		if err := decodePatch(patchRaw, &s); err != nil {
			return err
		}
		plan.DefinitionOfDone.CurrentState = s
	case "module_shape":
		var s string
		if err := decodePatch(patchRaw, &s); err != nil {
			return err
		}
		plan.DefinitionOfDone.ModuleShape = s
	default:
		return fmt.Errorf("invalid definition_of_done subsection %q: valid values are narrative, goals, current_state, module_shape", subsection)
	}
	return nil
}

func applyImplementationPatch(plan *schema.Plan, opts ReplaceOptions, patchRaw []byte) ([]int, bool, error) {
	if opts.Append {
		var step schema.Step
		if err := decodePatch(patchRaw, &step); err != nil {
			return nil, false, err
		}
		plan.Implementation = append(plan.Implementation, step)
		return []int{len(plan.Implementation)}, true, nil
	}
	if opts.Subsection != "" {
		idx, err := strconv.Atoi(opts.Subsection)
		if err != nil || idx < 1 || idx > len(plan.Implementation) {
			return nil, false, fmt.Errorf("invalid implementation step index %q: valid range is 1-%d", opts.Subsection, len(plan.Implementation))
		}
		var step schema.Step
		if err := decodePatch(patchRaw, &step); err != nil {
			return nil, false, err
		}
		plan.Implementation[idx-1] = step
		return []int{idx}, false, nil
	}
	var steps []schema.Step
	if err := decodePatch(patchRaw, &steps); err != nil {
		return nil, false, err
	}
	plan.Implementation = steps
	stepsReplaced := make([]int, len(steps))
	for i := range steps {
		stepsReplaced[i] = i + 1
	}
	return stepsReplaced, false, nil
}

// applyFieldPatch dispatches a field-level patch. Phase 4 ships --field diff
// only; PDEV-028 will add new cases by extending this switch.
func applyFieldPatch(source string, opts ReplaceOptions, patchRaw []byte, plan schema.Plan, diffSpans [][]inspect.Span) (string, ReplaceResult, error) {
	switch opts.Field {
	case "diff":
		return spliceDiffField(source, opts, patchRaw, plan, diffSpans)
	default:
		return "", ReplaceResult{}, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("unknown --field %q (valid: diff)", opts.Field))
	}
}

// spliceDiffField replaces one FileChange diff body with raw bytes and then
// re-parses the spliced markdown before returning. No JSON decode or schema
// validation runs on the field body itself.
func spliceDiffField(source string, opts ReplaceOptions, patchRaw []byte, plan schema.Plan, diffSpans [][]inspect.Span) (string, ReplaceResult, error) {
	stepIdx, err := strconv.Atoi(opts.Subsection)
	if err != nil || stepIdx < 1 || stepIdx > len(plan.Implementation) {
		return "", ReplaceResult{}, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("--subsection %q invalid for implementation (have %d steps)", opts.Subsection, len(plan.Implementation)))
	}

	matches := []int{}
	for i, fc := range plan.Implementation[stepIdx-1].FileChanges {
		if fc.Filename == opts.File {
			matches = append(matches, i)
		}
	}

	switch len(matches) {
	case 0:
		return "", ReplaceResult{}, newReplaceError(ReplaceFileNotFoundError, fmt.Errorf("--file %q not found in step %d", opts.File, stepIdx))
	case 1:
	default:
		return "", ReplaceResult{}, newReplaceError(ReplaceFileAmbiguousError, fmt.Errorf("--file %q matched %d FileChanges in step %d; consolidate or rename before patching", opts.File, len(matches), stepIdx))
	}

	span := diffSpans[stepIdx-1][matches[0]]
	out := splice(source, span, string(patchRaw))
	parsed, _, _, _, err := inspect.ParseMarkdown(out)
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceParseSplicedSourceError, fmt.Errorf("spliced diff body breaks plan parsing (likely contains a fence-like line): %w", err))
	}
	if err := validate.ValidatePlan(parsed); err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceValidateResultError, err)
	}

	return out, ReplaceResult{
		Section:    opts.Section,
		Subsection: opts.Subsection,
		File:       opts.File,
		Field:      opts.Field,
	}, nil
}

func decodePatch(patchRaw []byte, target any) error {
	return newReplaceError(ReplaceDecodePatchError, jsoninput.DecodeStrict(patchRaw, target))
}

func applySplice(source string, updated schema.Plan, opts ReplaceOptions, sectionSpans inspect.SectionSpans, stepSpans []inspect.Span) (string, error) {
	// Step-level replace: splice only the targeted step span.
	if opts.Section == "implementation" && !opts.Append && opts.Subsection != "" {
		idx, err := strconv.Atoi(opts.Subsection)
		if err != nil {
			return "", fmt.Errorf("invalid implementation step index %q", opts.Subsection)
		}
		renderedStep, err := render.RenderImplementationStep(idx, updated.Implementation[idx-1])
		if err != nil {
			return "", err
		}
		return splice(source, stepSpans[idx-1], renderedStep), nil
	}
	// Full section replace: re-render the whole plan, extract the target section,
	// then splice it into the source. This guarantees correct formatting without
	// requiring per-section templates.
	replacement, err := renderSection(updated, opts.Section)
	if err != nil {
		return "", err
	}
	var targetSpan inspect.Span
	switch opts.Section {
	case "overview":
		targetSpan = sectionSpans.Overview
	case "definition_of_done":
		targetSpan = sectionSpans.DefinitionOfDone
	case "implementation":
		targetSpan = sectionSpans.Implementation
	case "verification":
		targetSpan = sectionSpans.Verification
	}
	return splice(source, targetSpan, replacement), nil
}

// renderSection re-renders the full plan and extracts the named section. This
// avoids needing separate per-section templates while guaranteeing canonical output.
func renderSection(plan schema.Plan, section string) (string, error) {
	full, err := render.RenderPlan(plan)
	if err != nil {
		return "", err
	}
	_, spans, _, _, err := inspect.ParseMarkdown(full)
	if err != nil {
		return "", err
	}
	switch section {
	case "overview":
		return full[spans.Overview.Start:spans.Overview.End], nil
	case "definition_of_done":
		return full[spans.DefinitionOfDone.Start:spans.DefinitionOfDone.End], nil
	case "implementation":
		return full[spans.Implementation.Start:spans.Implementation.End], nil
	case "verification":
		return full[spans.Verification.Start:spans.Verification.End], nil
	default:
		return "", fmt.Errorf("unknown section %q", section)
	}
}

func splice(raw string, span inspect.Span, replacement string) string {
	return raw[:span.Start] + replacement + raw[span.End:]
}

func assertPreserved(before, after string, opts ReplaceOptions, beforeSections, afterSections inspect.SectionSpans, beforeSteps, afterSteps []inspect.Span) error {
	if opts.Section != "overview" {
		if rawAt(before, beforeSections.Overview) != rawAt(after, afterSections.Overview) {
			return fmt.Errorf("overview changed unexpectedly")
		}
	}
	if opts.Section != "definition_of_done" {
		if rawAt(before, beforeSections.DefinitionOfDone) != rawAt(after, afterSections.DefinitionOfDone) {
			return fmt.Errorf("definition_of_done changed unexpectedly")
		}
	}
	if opts.Section != "verification" {
		if rawAt(before, beforeSections.Verification) != rawAt(after, afterSections.Verification) {
			return fmt.Errorf("verification changed unexpectedly")
		}
	}

	// For step-level replace, verify non-targeted steps are preserved.
	if opts.Section == "implementation" && opts.Subsection != "" && !opts.Append {
		idx, _ := strconv.Atoi(opts.Subsection)
		if len(beforeSteps) != len(afterSteps) {
			return fmt.Errorf("implementation step count changed unexpectedly")
		}
		for i := range beforeSteps {
			if i == idx-1 {
				continue
			}
			if rawAt(before, beforeSteps[i]) != rawAt(after, afterSteps[i]) {
				return fmt.Errorf("implementation.%d changed unexpectedly", i+1)
			}
		}
	}

	return nil
}

func rawAt(raw string, span inspect.Span) string {
	if span.Start < 0 || span.End > len(raw) || span.Start >= span.End {
		return ""
	}
	return raw[span.Start:span.End]
}

// WriteAtomic writes data to path via a temp file + rename. Exported so the
// CLI preview path can commit after PreviewFromData without re-running the
// full replace pipeline.
func WriteAtomic(path string, data []byte) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return err
	}
	tmp, err := os.CreateTemp(filepath.Dir(path), ".planner-replace-*.tmp")
	if err != nil {
		return err
	}
	tmpPath := tmp.Name()
	if _, err := tmp.Write(data); err != nil {
		_ = tmp.Close()
		_ = os.Remove(tmpPath)
		return err
	}
	if err := tmp.Close(); err != nil {
		_ = os.Remove(tmpPath)
		return err
	}
	if err := os.Rename(tmpPath, path); err != nil {
		_ = os.Remove(tmpPath)
		return err
	}
	return nil
}
