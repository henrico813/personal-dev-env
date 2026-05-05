package internal

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
)

// ReplaceErrorCode classifies failures from PreviewFromData/Run so CLI callers
// can preserve stable error categories without guessing from strings.
type ReplaceErrorCode int

const (
	ReplaceInvalidOptionsError ReplaceErrorCode = iota + 1
	ReplaceReadSourceError
	ReplaceParseSourceError
	ReplaceDecodePatchError
	ReplaceValidateResultError
	ReplaceRenderResultError
	ReplaceFileNotFoundError
	ReplaceFileAmbiguousError
	ReplaceParseSplicedSourceError
)

// ReplaceError wraps the underlying failure with a stable category.
type ReplaceError struct {
	Code ReplaceErrorCode
	Err  error
}

func (e *ReplaceError) Error() string { return e.Err.Error() }

func (e *ReplaceError) Unwrap() error { return e.Err }

func newReplaceError(code ReplaceErrorCode, err error) error {
	if err == nil {
		return nil
	}
	return &ReplaceError{Code: code, Err: err}
}

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
	Change     int    // 1-based FileChange index inside an implementation step; preferred by behavioral edit CLI
	Field      string // field selector for implementation leaf updates
	Raw        bool   // raw scalar input for string targets; required for scalar string patch paths
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

	parsed, err := ParseMarkdown(string(sourceRaw))
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceParseSourceError, err)
	}
	plan := parsed.Plan
	sectionSpans := parsed.Sections
	stepSpans := parsed.Steps
	diffSpans := parsed.DiffContents

	if opts.Raw {
		return rawScalarPatch(string(sourceRaw), opts, string(patchRaw), plan, sectionSpans, stepSpans)
	}
	if opts.Field != "" {
		return applyFieldPatch(string(sourceRaw), opts, patchRaw, plan, stepSpans, diffSpans)
	}

	updated := plan
	stepsReplaced := []int{}
	appended := false
	switch opts.Section {
	case "title":
		return applyTitlePatch(string(sourceRaw), opts, patchRaw, sectionSpans)
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
		if err := applyVerificationPatch(&updated, opts.Subsection, patchRaw); err != nil {
			return "", ReplaceResult{}, err
		}
	}

	return finalizeUpdatedPlan(string(sourceRaw), updated, opts, sectionSpans, stepSpans, stepsReplaced, appended)
}

func isScalarOpts(opts ReplaceOptions) bool {
	switch opts.Section {
	case "title", "overview":
		return true
	case "definition_of_done":
		return opts.Subsection == "narrative" || opts.Subsection == "current_state" || opts.Subsection == "module_shape"
	case "implementation":
		switch opts.Field {
		case "title", "summary", "filename", "explanation":
			return true
		}
	case "verification":
		return opts.Subsection == "summary"
	}
	return false
}

func validateOpts(opts ReplaceOptions) error {
	switch opts.Section {
	case "title", "overview", "definition_of_done", "implementation", "verification":
	default:
		return fmt.Errorf("invalid section %q: valid values are title, overview, definition_of_done, implementation, verification", opts.Section)
	}
	if opts.Section == "title" {
		if opts.Subsection != "" || opts.File != "" || opts.Field != "" || opts.Append {
			return fmt.Errorf("--section title accepts no other selectors")
		}
		if !opts.Raw {
			return fmt.Errorf("scalar string targets require --raw (JSON string input is no longer accepted on this path)")
		}
		return nil
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
	if opts.Section == "verification" && opts.Subsection != "" {
		switch opts.Subsection {
		case "summary", "automated", "manual":
		default:
			return fmt.Errorf("invalid verification subsection %q: valid values are summary, automated, manual", opts.Subsection)
		}
	}
	if opts.Section == "overview" && opts.Subsection != "" {
		return fmt.Errorf("--subsection is not supported for section %q", opts.Section)
	}
	if opts.Field != "" {
		if opts.Section != "implementation" {
			return fmt.Errorf("--field requires --section implementation")
		}
		if opts.Subsection == "" {
			return fmt.Errorf("--field requires --subsection N")
		}
		switch opts.Field {
		case "diff", "filename", "explanation":
			if opts.File == "" && opts.Change == 0 {
				return fmt.Errorf("--field %s requires --file F or --change N", opts.Field)
			}
		case "title", "summary":
			if opts.File != "" {
				return fmt.Errorf("--field %s does not take --file", opts.Field)
			}
		default:
			return fmt.Errorf("unknown --field %q (valid: diff, title, summary, filename, explanation)", opts.Field)
		}
	}
	if opts.File != "" && opts.Field == "" {
		return fmt.Errorf("--file requires --field")
	}
	if opts.Raw && !isScalarOpts(opts) {
		return fmt.Errorf("--raw is only valid with scalar string targets")
	}
	if !opts.Raw && isScalarOpts(opts) {
		return fmt.Errorf("scalar string targets require --raw (JSON string input is no longer accepted on this path)")
	}
	return nil
}

func applyDoDPatch(plan *Plan, subsection string, patchRaw []byte) error {
	if subsection == "" {
		var dod DefinitionOfDone
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
		var goals []ChecklistItem
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

func applyImplementationPatch(plan *Plan, opts ReplaceOptions, patchRaw []byte) ([]int, bool, error) {
	if opts.Append {
		var step Step
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
		var step Step
		if err := decodePatch(patchRaw, &step); err != nil {
			return nil, false, err
		}
		plan.Implementation[idx-1] = step
		return []int{idx}, false, nil
	}
	var steps []Step
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

// finalizeFieldPatch reruns the post-splice integrity gate used by the field
// patch paths: parse the spliced markdown, then validate the reconstructed
// plan before returning success.
func finalizeFieldPatch(out string, opts ReplaceOptions) (string, ReplaceResult, error) {
	parsed, err := ParseMarkdown(out)
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceParseSplicedSourceError, err)
	}
	if err := ValidatePlan(parsed.Plan); err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceValidateResultError, err)
	}
	return out, ReplaceResult{
		Section:    opts.Section,
		Subsection: opts.Subsection,
		File:       opts.File,
		Field:      opts.Field,
	}, nil
}

// lookupFileChange returns the FileChange index in the addressed step whose
// filename matches opts.File.
func lookupFileChange(plan Plan, opts ReplaceOptions, stepIdx int) (int, error) {
	if opts.Change != 0 {
		if opts.Change < 1 || opts.Change > len(plan.Implementation[stepIdx-1].FileChanges) {
			return 0, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("--change %d out of range for step %d", opts.Change, stepIdx))
		}
		return opts.Change - 1, nil
	}
	matches := []int{}
	for i, fc := range plan.Implementation[stepIdx-1].FileChanges {
		if fc.Filename == opts.File {
			matches = append(matches, i)
		}
	}
	switch len(matches) {
	case 0:
		return 0, newReplaceError(ReplaceFileNotFoundError, fmt.Errorf("--file %q not found in step %d", opts.File, stepIdx))
	case 1:
		return matches[0], nil
	default:
		return 0, newReplaceError(ReplaceFileAmbiguousError, fmt.Errorf("--file %q matched %d FileChanges in step %d; consolidate or rename before patching", opts.File, len(matches), stepIdx))
	}
}

// requireStepIndex parses opts.Subsection as a 1-based implementation step
// index and returns the addressed step number.
func requireStepIndex(opts ReplaceOptions, plan Plan) (int, error) {
	idx, err := strconv.Atoi(opts.Subsection)
	if err != nil || idx < 1 || idx > len(plan.Implementation) {
		return 0, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("--subsection %q invalid for implementation (have %d steps)", opts.Subsection, len(plan.Implementation)))
	}
	return idx, nil
}

func spliceStepField(source string, opts ReplaceOptions, patchRaw []byte, plan Plan, stepSpans []Span) (string, ReplaceResult, error) {
	stepIdx, err := requireStepIndex(opts, plan)
	if err != nil {
		return "", ReplaceResult{}, err
	}
	var value string
	if err := decodePatch(patchRaw, &value); err != nil {
		return "", ReplaceResult{}, err
	}
	updated := plan.Implementation[stepIdx-1]
	switch opts.Field {
	case "title":
		updated.Title = value
	case "summary":
		updated.Summary = value
	default:
		return "", ReplaceResult{}, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("spliceStepField got unexpected --field %q", opts.Field))
	}
	rendered, err := RenderImplementationStep(stepIdx, updated)
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceRenderResultError, err)
	}
	return finalizeFieldPatch(splice(source, stepSpans[stepIdx-1], rendered), opts)
}

func spliceFileChangeField(source string, opts ReplaceOptions, patchRaw []byte, plan Plan, stepSpans []Span) (string, ReplaceResult, error) {
	stepIdx, err := requireStepIndex(opts, plan)
	if err != nil {
		return "", ReplaceResult{}, err
	}
	fcIdx, err := lookupFileChange(plan, opts, stepIdx)
	if err != nil {
		return "", ReplaceResult{}, err
	}
	var value string
	if err := decodePatch(patchRaw, &value); err != nil {
		return "", ReplaceResult{}, err
	}
	updated := plan.Implementation[stepIdx-1]
	fc := updated.FileChanges[fcIdx]
	switch opts.Field {
	case "filename":
		fc.Filename = value
	case "explanation":
		fc.Explanation = value
	default:
		return "", ReplaceResult{}, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("spliceFileChangeField got unexpected --field %q", opts.Field))
	}
	updated.FileChanges[fcIdx] = fc
	rendered, err := RenderImplementationStep(stepIdx, updated)
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceRenderResultError, err)
	}
	return finalizeFieldPatch(splice(source, stepSpans[stepIdx-1], rendered), opts)
}

func applyTitlePatch(source string, opts ReplaceOptions, patchRaw []byte, sectionSpans SectionSpans) (string, ReplaceResult, error) {
	var value string
	if err := decodePatch(patchRaw, &value); err != nil {
		return "", ReplaceResult{}, err
	}
	return finalizeFieldPatch(splice(source, sectionSpans.Title, value), opts)
}

func applyVerificationPatch(plan *Plan, subsection string, patchRaw []byte) error {
	if subsection == "" {
		var v Verification
		if err := decodePatch(patchRaw, &v); err != nil {
			return err
		}
		plan.Verification = &v
		return nil
	}
	if plan.Verification == nil {
		plan.Verification = &Verification{}
	}
	switch subsection {
	case "summary":
		var s string
		if err := decodePatch(patchRaw, &s); err != nil {
			return err
		}
		plan.Verification.Summary = s
	case "automated":
		var items []ChecklistItem
		if err := decodePatch(patchRaw, &items); err != nil {
			return err
		}
		plan.Verification.Automated = items
	case "manual":
		var items []ChecklistItem
		if err := decodePatch(patchRaw, &items); err != nil {
			return err
		}
		plan.Verification.Manual = items
	default:
		return fmt.Errorf("invalid verification subsection %q: valid values are summary, automated, manual", subsection)
	}
	return nil
}

func finalizeUpdatedPlan(source string, updated Plan, opts ReplaceOptions, sectionSpans SectionSpans, stepSpans []Span, stepsReplaced []int, appended bool) (string, ReplaceResult, error) {
	if err := ValidatePlan(updated); err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceValidateResultError, err)
	}

	out, err := applySplice(source, updated, opts, sectionSpans, stepSpans)
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceRenderResultError, err)
	}

	reparsed, err := ParseMarkdown(out)
	if err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceRenderResultError, err)
	}
	if err := assertPreserved(source, out, opts, sectionSpans, reparsed.Sections, stepSpans, reparsed.Steps); err != nil {
		return "", ReplaceResult{}, newReplaceError(ReplaceValidateResultError, err)
	}

	return out, ReplaceResult{
		Section:       opts.Section,
		Subsection:    opts.Subsection,
		StepsReplaced: stepsReplaced,
		Appended:      appended,
	}, nil
}

// rawScalarPatch assigns raw text to scalar string targets after the caller
// strips the trailing newline. It keeps the same post-splice validation path
// as the JSON-backed patch flow.
func rawScalarPatch(source string, opts ReplaceOptions, value string, plan Plan, sectionSpans SectionSpans, stepSpans []Span) (string, ReplaceResult, error) {
	switch opts.Section {
	case "title":
		return finalizeFieldPatch(splice(source, sectionSpans.Title, value), opts)
	case "overview":
		updated := plan
		updated.Overview = value
		return finalizeUpdatedPlan(source, updated, opts, sectionSpans, stepSpans, nil, false)
	case "definition_of_done":
		updated := plan
		switch opts.Subsection {
		case "narrative":
			updated.DefinitionOfDone.Narrative = value
		case "current_state":
			updated.DefinitionOfDone.CurrentState = value
		case "module_shape":
			updated.DefinitionOfDone.ModuleShape = value
		default:
			return "", ReplaceResult{}, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("invalid definition_of_done subsection %q: valid values are narrative, goals, current_state, module_shape", opts.Subsection))
		}
		return finalizeUpdatedPlan(source, updated, opts, sectionSpans, stepSpans, nil, false)
	case "implementation":
		stepIdx, err := requireStepIndex(opts, plan)
		if err != nil {
			return "", ReplaceResult{}, err
		}
		updated := plan.Implementation[stepIdx-1]
		switch opts.Field {
		case "title":
			updated.Title = value
		case "summary":
			updated.Summary = value
		case "filename":
			fcIdx, err := lookupFileChange(plan, opts, stepIdx)
			if err != nil {
				return "", ReplaceResult{}, err
			}
			fc := updated.FileChanges[fcIdx]
			fc.Filename = value
			updated.FileChanges[fcIdx] = fc
		case "explanation":
			fcIdx, err := lookupFileChange(plan, opts, stepIdx)
			if err != nil {
				return "", ReplaceResult{}, err
			}
			fc := updated.FileChanges[fcIdx]
			fc.Explanation = value
			updated.FileChanges[fcIdx] = fc
		default:
			return "", ReplaceResult{}, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("invalid raw scalar field %q", opts.Field))
		}
		rendered, err := RenderImplementationStep(stepIdx, updated)
		if err != nil {
			return "", ReplaceResult{}, newReplaceError(ReplaceRenderResultError, err)
		}
		return finalizeFieldPatch(splice(source, stepSpans[stepIdx-1], rendered), opts)
	case "verification":
		updated := plan
		if updated.Verification == nil {
			updated.Verification = &Verification{}
		}
		if opts.Subsection != "summary" {
			return "", ReplaceResult{}, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("invalid verification subsection %q: valid values are summary, automated, manual", opts.Subsection))
		}
		updated.Verification.Summary = value
		return finalizeUpdatedPlan(source, updated, opts, sectionSpans, stepSpans, nil, false)
	default:
		return "", ReplaceResult{}, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("invalid raw scalar patch target for section %q", opts.Section))
	}
}

// applyFieldPatch dispatches a field-level patch across diff, step leaves, and
// file-change leaves. The caller already validated the selector grammar.
func applyFieldPatch(source string, opts ReplaceOptions, patchRaw []byte, plan Plan, stepSpans []Span, diffSpans [][]Span) (string, ReplaceResult, error) {
	switch opts.Field {
	case "diff":
		return spliceDiffField(source, opts, patchRaw, plan, diffSpans)
	case "title", "summary":
		return spliceStepField(source, opts, patchRaw, plan, stepSpans)
	case "filename", "explanation":
		return spliceFileChangeField(source, opts, patchRaw, plan, stepSpans)
	default:
		return "", ReplaceResult{}, newReplaceError(ReplaceInvalidOptionsError, fmt.Errorf("unknown --field %q (valid: diff, title, summary, filename, explanation)", opts.Field))
	}
}

// spliceDiffField replaces one FileChange diff body with raw bytes and then
// re-parses the spliced markdown before returning. No JSON decode or schema
// validation runs on the field body itself.
func spliceDiffField(source string, opts ReplaceOptions, patchRaw []byte, plan Plan, diffSpans [][]Span) (string, ReplaceResult, error) {
	stepIdx, err := requireStepIndex(opts, plan)
	if err != nil {
		return "", ReplaceResult{}, err
	}
	fcIdx, err := lookupFileChange(plan, opts, stepIdx)
	if err != nil {
		return "", ReplaceResult{}, err
	}

	span := diffSpans[stepIdx-1][fcIdx]
	return finalizeFieldPatch(splice(source, span, string(patchRaw)), opts)
}

func decodePatch(patchRaw []byte, target any) error {
	return newReplaceError(ReplaceDecodePatchError, DecodeStrict(patchRaw, target))
}

func applySplice(source string, updated Plan, opts ReplaceOptions, sectionSpans SectionSpans, stepSpans []Span) (string, error) {
	// Step-level replace: splice only the targeted step span.
	if opts.Section == "implementation" && !opts.Append && opts.Subsection != "" {
		idx, err := strconv.Atoi(opts.Subsection)
		if err != nil {
			return "", fmt.Errorf("invalid implementation step index %q", opts.Subsection)
		}
		renderedStep, err := RenderImplementationStep(idx, updated.Implementation[idx-1])
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
	var targetSpan Span
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
func renderSection(plan Plan, section string) (string, error) {
	full, err := RenderPlan(plan)
	if err != nil {
		return "", err
	}
	preserved, err := ParseMarkdown(full)
	if err != nil {
		return "", err
	}
	spans := preserved.Sections
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

func splice(raw string, span Span, replacement string) string {
	return raw[:span.Start] + replacement + raw[span.End:]
}

func assertPreserved(before, after string, opts ReplaceOptions, beforeSections, afterSections SectionSpans, beforeSteps, afterSteps []Span) error {
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

func rawAt(raw string, span Span) string {
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
