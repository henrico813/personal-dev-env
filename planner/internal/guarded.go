package internal

import (
	"bytes"
	"crypto/sha256"
	"errors"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"reflect"
	"regexp"
	"strconv"
	"strings"

	"planner/internal/planpatch"
)

// Guarded-specific failure codes. Shared categories (USAGE, READ_INPUT,
// DECODE_INPUT, VALIDATE_INPUT, WRITE_OUTPUT) are defined in errors.go, and the
// guarded emitter reads their registered names through plannerCode.
const (
	codeSourceCheck        = "SOURCE_CHECK"
	codePatchInput         = "PATCH_INPUT"
	codeValidateResult     = "VALIDATE_RESULT"
	codePlanEdit           = "PLAN_EDIT"
	codeCollateralChange   = "PLAN_COLLATERAL_CHANGE"
	codeOutputReportFailed = "OUTPUT_REPORT_FAILED"
	// codeBaseCommitRequired marks a plan that records no base commit and a check
	// call that supplied no --base-commit. It is distinct from planpatch's
	// CodeBaseCommitInvalid so the recovery hint can name the Current State line.
	codeBaseCommitRequired = "BASE_COMMIT_REQUIRED"
)

// plannerCode returns the registered string for a shared PlannerErrorCode.
func plannerCode(code PlannerErrorCode) string { return plannerErrorCodeNames[code] }

// usageError tags an invalid option combination as USAGE without involving the
// CLI flag grammar, so each options type can validate itself.
func usageError(message string) error {
	return &planpatch.Error{Code: plannerCode(PlannerUsageError), Cause: errors.New(message)}
}

// codedError tags an untagged error with a default code. Errors that already
// carry a planpatch code keep it so a specific failure is not flattened.
func codedError(code string, err error) error {
	var patchErr *planpatch.Error
	if errors.As(err, &patchErr) {
		return err
	}
	return &planpatch.Error{Code: code, Cause: err}
}

// guardedInspectOptions selects one fenced change and optionally writes the
// proposed source that precedes or follows it. Repo and BaseCommit are required
// so the returned edit_expect binds the recorded base commit.
type guardedInspectOptions struct {
	PlanPath   string
	Target     string
	Repo       string
	BaseCommit string
	CodeOut    string
	Before     bool
}

func (o guardedInspectOptions) validate() error {
	switch {
	case o.Target == "":
		return usageError("--target is required")
	case o.Repo == "" || o.BaseCommit == "":
		return usageError("--repo and --base-commit are required")
	case o.Before && o.CodeOut == "":
		return usageError("--before requires --code-out")
	}
	return nil
}

// guardedInspectResult is the JSON view returned to a revision caller. The
// code_* fields are present only when --code-out wrote source.
type guardedInspectResult struct {
	Selector    string `json:"selector"`
	Filename    string `json:"filename"`
	StepTitle   string `json:"step_title"`
	StepSummary string `json:"step_summary"`
	Explanation string `json:"explanation"`
	BaseCommit  string `json:"base_commit"`
	EditExpect  string `json:"edit_expect"`
	Validation  string `json:"validation"`
	Diff        string `json:"diff,omitempty"`
	CodeExists  *bool  `json:"code_exists,omitempty"`
	CodeState   string `json:"code_state,omitempty"`
	Mode        string `json:"mode,omitempty"`
	CodeOut     string `json:"code_out,omitempty"`
}

func guardedInspect(opts guardedInspectOptions) (guardedInspectResult, error) {
	var result guardedInspectResult
	if err := opts.validate(); err != nil {
		return result, err
	}
	raw, err := readGuardedInput(opts.PlanPath)
	if err != nil {
		return result, codedError(plannerCode(PlannerReadInputError), err)
	}
	parsed, err := ParseMarkdown(string(raw))
	if err != nil {
		return result, codedError(plannerCode(PlannerDecodeInputError), err)
	}
	step, change, selector, err := selectedChange(parsed, opts.Target)
	if err != nil {
		return result, codedError(plannerCode(PlannerUsageError), err)
	}
	selected := parsed.Plan.Implementation[step].FileChanges[change]
	result = guardedInspectResult{
		Selector:    selector,
		Filename:    selected.Filename,
		StepTitle:   parsed.Plan.Implementation[step].Title,
		StepSummary: parsed.Plan.Implementation[step].Summary,
		Explanation: selected.Explanation,
		BaseCommit:  opts.BaseCommit,
		EditExpect:  planpatch.Expect(raw, selector, opts.BaseCommit),
		Validation:  "inspection_only",
	}
	if opts.CodeOut == "" {
		result.Diff = selected.Diff
		return result, nil
	}
	s, err := sessionBefore(opts.Repo, opts.BaseCommit, parsed.Plan, step, change)
	if err != nil {
		return result, codedError(codeSourceCheck, err)
	}
	defer s.Close()
	if !opts.Before {
		err = s.Apply(planpatch.Change{
			Target:   selector,
			Filename: selected.Filename,
			Diff:     []byte(selected.Diff),
		})
		if err != nil {
			return result, codedError(codeSourceCheck, err)
		}
	}
	file, err := s.Read(selected.Filename)
	if err != nil {
		return result, codedError(codeSourceCheck, err)
	}
	var code []byte
	exists := file != nil
	if file != nil {
		code = file.Data
		result.Mode = file.Mode
	}
	result.CodeExists = &exists
	result.CodeState = "after_selected_change"
	if opts.Before {
		result.CodeState = "before_selected_change"
	}
	if err := writeNewScratch(opts.CodeOut, code); err != nil {
		return result, codedError(plannerCode(PlannerWriteOutputError), err)
	}
	result.CodeOut = opts.CodeOut
	return result, nil
}

// guardedPatchOptions replaces one fenced change from ordinary source or a raw
// diff, guarded by the edit_expect token and mandatory repository apply.
type guardedPatchOptions struct {
	PlanPath   string
	Target     string
	Expect     string
	AfterFile  string
	DiffFile   string
	Repo       string
	BaseCommit string
	DryRun     bool
	Diff       bool
}

func (o guardedPatchOptions) validate() error {
	switch {
	case o.Target == "":
		return usageError("--target is required")
	case o.Expect == "":
		return usageError("--expect is required")
	case o.Repo == "" || o.BaseCommit == "":
		return usageError("--repo and --base-commit are required")
	case (o.AfterFile == "") == (o.DiffFile == ""):
		return usageError("exactly one of --after-file and --diff-file is required")
	}
	return nil
}

// guardedPatchResult reports what was checked and whether the plan was written.
// Preview is the Git-generated review delta for --diff and is not JSON encoded.
type guardedPatchResult struct {
	Path              string `json:"path"`
	PlanSHA256        string `json:"plan_sha256"`
	Written           bool   `json:"written"`
	Changed           bool   `json:"changed"`
	StructureValid    bool   `json:"structure_valid"`
	PatchSyntaxValid  bool   `json:"patch_syntax_valid"`
	PrefixReplayed    bool   `json:"prefix_replayed"`
	DownstreamChecked bool   `json:"downstream_checked"`
	BaseCommit        string `json:"base_commit"`
	BehaviorChecked   bool   `json:"behavior_checked"`
	Preview           []byte `json:"-"`
}

func guardedPatch(opts guardedPatchOptions) (guardedPatchResult, error) {
	var result guardedPatchResult
	if err := opts.validate(); err != nil {
		return result, err
	}
	raw, err := readGuardedInput(opts.PlanPath)
	if err != nil {
		return result, codedError(plannerCode(PlannerReadInputError), err)
	}
	parsed, err := ParseMarkdown(string(raw))
	if err != nil {
		return result, codedError(plannerCode(PlannerDecodeInputError), err)
	}
	step, change, selector, err := selectedChange(parsed, opts.Target)
	if err != nil {
		return result, codedError(plannerCode(PlannerUsageError), err)
	}
	// The token binds the plan bytes, the normalized selector, and the base
	// commit. Now that inspect and patch always have a base commit, any mismatch
	// is PLAN_STALE.
	if opts.Expect != planpatch.Expect(raw, selector, opts.BaseCommit) {
		return result, &planpatch.Error{
			Code:  planpatch.CodePlanStale,
			Cause: errors.New("plan, target, or base commit changed since inspection"),
		}
	}
	selected := parsed.Plan.Implementation[step].FileChanges[change]
	s, err := sessionBefore(opts.Repo, opts.BaseCommit, parsed.Plan, step, change)
	if err != nil {
		return result, codedError(codeSourceCheck, err)
	}
	defer s.Close()
	var replacement []byte
	if opts.AfterFile != "" {
		before, err := s.Read(selected.Filename)
		if err != nil {
			return result, codedError(codeSourceCheck, err)
		}
		var after *planpatch.File
		if opts.AfterFile != os.DevNull {
			data, err := readGuardedInput(opts.AfterFile)
			if err != nil {
				return result, codedError(plannerCode(PlannerReadInputError), err)
			}
			mode := "100644"
			if before != nil {
				mode = before.Mode
			}
			matchesBefore := before == nil && len(data) == 0
			if before != nil {
				matchesBefore = bytes.Equal(data, before.Data)
			}
			if strings.TrimSpace(selected.Diff) == "PLACEHOLDER" && matchesBefore {
				return result, codedError(codePatchInput, errors.New(
					"after-file is identical to the --before file from inspect; edit it first"))
			}
			after = &planpatch.File{Data: data, Mode: mode}
		}
		replacement, err = planpatch.Generate(selected.Filename, before, after)
		if err != nil {
			return result, codedError(codePatchInput, err)
		}
	} else {
		replacement, err = readGuardedInput(opts.DiffFile)
		if err != nil {
			return result, codedError(codePatchInput, err)
		}
	}
	// Prefix apply: apply the edited change on top of the base commit plus every
	// earlier change. Later changes are deliberately not applied; only
	// planner check --repo --base-commit validates the whole plan for readiness.
	if err := s.Apply(planpatch.Change{
		Target:   selector,
		Filename: selected.Filename,
		Diff:     replacement,
	}); err != nil {
		return result, codedError(codeSourceCheck, err)
	}
	span := parsed.DiffContents[step][change]
	updated, err := planpatch.ReplaceDiff(raw, span.Start, span.End, replacement)
	if err != nil {
		return result, codedError(codePlanEdit, err)
	}
	candidate, err := ParseMarkdown(string(updated))
	if err != nil {
		return result, codedError(plannerCode(PlannerDecodeInputError), err)
	}
	if !reflect.DeepEqual(expectedPlan(parsed.Plan, step, change, replacement), candidate.Plan) {
		return result, &planpatch.Error{
			Code:  codeCollateralChange,
			Cause: errors.New("replacement changed another parsed field"),
		}
	}
	changed := !bytes.Equal(raw, updated)
	if err := validateGuardedPlan(candidate.Plan); err != nil {
		return result, codedError(codeValidateResult, err)
	}
	if opts.Diff && changed {
		preview, err := planpatch.Generate("plan.md",
			&planpatch.File{Data: raw, Mode: "100644"},
			&planpatch.File{Data: updated, Mode: "100644"})
		if err != nil {
			return result, codedError(codePatchInput, err)
		}
		result.Preview = preview
	} else if opts.Diff {
		result.Preview = []byte("No changes.\n")
	}
	if !opts.DryRun && changed {
		if err := planpatch.WriteIfUnchanged(opts.PlanPath, raw, updated); err != nil {
			return result, codedError(plannerCode(PlannerWriteOutputError), err)
		}
	}
	result.Path = opts.PlanPath
	result.PlanSHA256 = fmt.Sprintf("%x", sha256.Sum256(updated))
	result.Written = !opts.DryRun && changed
	result.Changed = changed
	result.StructureValid = true
	result.PatchSyntaxValid = true
	result.PrefixReplayed = true
	result.DownstreamChecked = false
	result.BaseCommit = opts.BaseCommit
	result.BehaviorChecked = false
	return result, nil
}

// expectedPlan is the pre-edit plan with only the selected diff replaced, used
// to detect a fence collision that rewrote an unrelated parsed field.
func expectedPlan(plan Plan, step, change int, replacement []byte) Plan {
	expected := plan
	expected.Implementation = append([]Step(nil), plan.Implementation...)
	expected.Implementation[step].FileChanges =
		append([]FileChange(nil), plan.Implementation[step].FileChanges...)
	expected.Implementation[step].FileChanges[change].Diff =
		strings.TrimRight(string(replacement), "\n")
	return expected
}

type guardedCheckOptions struct {
	PlanPath   string
	Repo       string
	BaseCommit string
}

type guardedCheckResult struct {
	PlanSHA256           string `json:"plan_sha256"`
	StructureValid       bool   `json:"structure_valid"`
	ApplicabilityChecked bool   `json:"applicability_checked"`
	ChangesReplayed      int    `json:"changes_replayed"`
	BaseCommit           string `json:"base_commit"`
	SourceState          string `json:"source_state"`
	BehaviorChecked      bool   `json:"behavior_checked"`
}

func guardedCheck(opts guardedCheckOptions) (guardedCheckResult, error) {
	var result guardedCheckResult
	raw, err := readGuardedInput(opts.PlanPath)
	if err != nil {
		return result, codedError(plannerCode(PlannerReadInputError), err)
	}
	parsed, err := ParseMarkdown(string(raw))
	if err != nil {
		// Reuse the markdown decoder's wrapped-doc subject and recovery hint so
		// check reports the same guidance as the other read commands.
		return result, plannerMarkdownDecodeError(raw, err)
	}
	if err := validateGuardedPlan(parsed.Plan); err != nil {
		return result, codedError(plannerCode(PlannerValidateInputError), err)
	}
	baseCommit, err := checkBaseCommit(opts.BaseCommit, parsed.Plan)
	if err != nil {
		return result, err
	}
	repo := opts.Repo
	if repo == "" {
		if repo, err = os.Getwd(); err != nil {
			return result, codedError(plannerCode(PlannerReadInputError), err)
		}
	}
	changes := orderedChanges(parsed.Plan)
	session, err := planpatch.ApplyToBase(repo, baseCommit, changes)
	if err != nil {
		return result, codedError(codeSourceCheck, err)
	}
	defer session.Close()
	result.PlanSHA256 = fmt.Sprintf("%x", sha256.Sum256(raw))
	result.StructureValid = true
	result.ApplicabilityChecked = true
	result.ChangesReplayed = len(changes)
	result.BaseCommit = baseCommit
	result.SourceState = "committed_snapshot_only"
	result.BehaviorChecked = false
	return result, nil
}

type guardedExportOptions struct {
	PlanPath   string
	Repo       string
	BaseCommit string
	Out        string
	Through    string
}

type guardedExportResult struct {
	Out        string `json:"out"`
	BaseCommit string `json:"base_commit"`
	Steps      int    `json:"steps"`
	Changes    int    `json:"changes_applied"`
}

func guardedExport(opts guardedExportOptions) (guardedExportResult, error) {
	var result guardedExportResult
	if opts.Repo == "" || opts.Out == "" {
		return result, usageError("--repo and --out are required")
	}
	if _, err := os.Lstat(opts.Out); err == nil {
		return result, codedError(plannerCode(PlannerWriteOutputError),
			fmt.Errorf("--out %s already exists; choose a new directory", opts.Out))
	} else if !errors.Is(err, os.ErrNotExist) {
		return result, codedError(plannerCode(PlannerReadInputError), err)
	}
	raw, err := readGuardedInput(opts.PlanPath)
	if err != nil {
		return result, codedError(plannerCode(PlannerReadInputError), err)
	}
	parsed, err := ParseMarkdown(string(raw))
	if err != nil {
		return result, plannerMarkdownDecodeError(raw, err)
	}
	if err := validateGuardedPlan(parsed.Plan); err != nil {
		return result, codedError(plannerCode(PlannerValidateInputError), err)
	}
	baseCommit, err := checkBaseCommit(opts.BaseCommit, parsed.Plan)
	if err != nil {
		return result, err
	}
	steps := len(parsed.Plan.Implementation)
	if opts.Through != "" {
		steps, err = strconv.Atoi(opts.Through)
		if err != nil || steps < 1 || steps > len(parsed.Plan.Implementation) {
			return result, usageError(fmt.Sprintf(
				"--through must be a step number from 1 to %d", len(parsed.Plan.Implementation)))
		}
	}
	changes := orderedChanges(Plan{Implementation: parsed.Plan.Implementation[:steps]})
	session, err := planpatch.ApplyToBase(opts.Repo, baseCommit, changes)
	if err != nil {
		return result, codedError(codeSourceCheck, err)
	}
	defer session.Close()
	if _, err := os.Lstat(opts.Out); err == nil {
		return result, codedError(plannerCode(PlannerWriteOutputError),
			fmt.Errorf("--out %s already exists; choose a new directory", opts.Out))
	} else if !errors.Is(err, os.ErrNotExist) {
		return result, codedError(plannerCode(PlannerReadInputError), err)
	}
	parent := filepath.Dir(opts.Out)
	temp, err := os.MkdirTemp(parent, ".planner-export-*")
	if err != nil {
		return result, codedError(plannerCode(PlannerWriteOutputError), err)
	}
	defer func() { _ = os.RemoveAll(temp) }()
	if err := session.Export(temp); err != nil {
		return result, codedError(plannerCode(PlannerWriteOutputError), err)
	}
	if _, err := os.Lstat(opts.Out); err == nil {
		return result, codedError(plannerCode(PlannerWriteOutputError),
			fmt.Errorf("--out %s already exists; choose a new directory", opts.Out))
	} else if !errors.Is(err, os.ErrNotExist) {
		return result, codedError(plannerCode(PlannerReadInputError), err)
	}
	if err := os.Rename(temp, opts.Out); err != nil {
		return result, codedError(plannerCode(PlannerWriteOutputError), err)
	}
	result = guardedExportResult{Out: opts.Out, BaseCommit: baseCommit,
		Steps: steps, Changes: len(changes)}
	return result, nil
}

// baseCommitRE matches the required first line of Current State. A full
// lowercase object ID keeps the recorded base commit immutable and unambiguous.
var baseCommitRE = regexp.MustCompile(`^Base commit: ([0-9a-f]{40}|[0-9a-f]{64})$`)

// checkBaseCommit returns the commit that check applies. An explicit
// --base-commit wins; otherwise the plan must record one on the first line of
// Current State. check never falls back to HEAD, so a stale plan cannot
// silently measure against the current checkout.
func checkBaseCommit(flagBaseCommit string, plan Plan) (string, error) {
	if flagBaseCommit != "" {
		return flagBaseCommit, nil
	}
	first, _, _ := strings.Cut(strings.TrimSpace(plan.DefinitionOfDone.CurrentState), "\n")
	if m := baseCommitRE.FindStringSubmatch(strings.TrimSpace(first)); m != nil {
		return m[1], nil
	}
	return "", &planpatch.Error{
		Code: codeBaseCommitRequired,
		Cause: errors.New(
			`first line of Current State must be "Base commit: <full commit ID>" ` +
				`or --base-commit must be given`),
	}
}

var patchFileChangeSelectorRE = regexp.MustCompile(`^implementation\[(-?\d+)\]\.file_changes\[(-?\d+)\]$`)

func parsePatchFileChangeSelector(selector string) (int, int, error) {
	match := patchFileChangeSelectorRE.FindStringSubmatch(selector)
	if match == nil {
		return 0, 0, fmt.Errorf("unsupported patch selector %q", selector)
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

func parsePatchSelectorIndex(raw, selector, segment string) (int, error) {
	idx, err := strconv.Atoi(raw)
	if err != nil {
		return 0, fmt.Errorf("unsupported patch selector %q", selector)
	}
	if idx < 1 {
		return 0, patchSelectorRangeError(selector, segment, idx, 0)
	}
	return idx, nil
}

func patchSelectorRangeError(selector, segment string, idx, have int) error {
	return fmt.Errorf("patch selector %q %s %d out of range (have %d)", selector, segment, idx, have)
}

// selectedChange resolves selector to zero-based plan indices and its
// normalized spelling. Indices are parsed numerically, so leading zeros address
// the same change and normalize to one spelling for tokens, results, and apply.
func selectedChange(parsed ParseResult, selector string) (step, change int, normalized string, err error) {
	step, change, err = parsePatchFileChangeSelector(selector)
	if err != nil {
		return 0, 0, "", err
	}
	if step > len(parsed.Plan.Implementation) ||
		change > len(parsed.Plan.Implementation[step-1].FileChanges) {
		return 0, 0, "", fmt.Errorf("target out of range: %s", selector)
	}
	return step - 1, change - 1,
		fmt.Sprintf("implementation[%d].file_changes[%d]", step, change), nil
}

func orderedChanges(plan Plan) []planpatch.Change {
	var out []planpatch.Change
	for i, step := range plan.Implementation {
		for j, change := range step.FileChanges {
			out = append(out, planpatch.Change{
				Target:   fmt.Sprintf("implementation[%d].file_changes[%d]", i+1, j+1),
				Filename: change.Filename,
				Diff:     []byte(change.Diff),
			})
		}
	}
	return out
}

// sessionBefore starts from the base commit and applies every change
// before (step, change), returning an open session just before the selected
// change. The caller owns the session and must Close it. Comparison is by parsed
// indices, so a selector with leading zeros selects the same change.
func sessionBefore(repo, baseCommit string, plan Plan, step, change int) (*planpatch.Session, error) {
	s, err := planpatch.Open(repo, baseCommit)
	if err != nil {
		return nil, err
	}
	for i, planStep := range plan.Implementation {
		for j, fileChange := range planStep.FileChanges {
			if i == step && j == change {
				return s, nil
			}
			if err := s.Apply(planpatch.Change{
				Target:   fmt.Sprintf("implementation[%d].file_changes[%d]", i+1, j+1),
				Filename: fileChange.Filename,
				Diff:     []byte(fileChange.Diff),
			}); err != nil {
				s.Close()
				return nil, err
			}
		}
	}
	s.Close()
	return nil, fmt.Errorf("target out of range: implementation[%d].file_changes[%d]",
		step+1, change+1)
}

// writeNewScratch never overwrites a file, including a symlink or the plan.
func writeNewScratch(name string, raw []byte) error {
	f, err := os.OpenFile(name, os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if err != nil {
		return err
	}
	_, err = f.Write(raw)
	closeErr := f.Close()
	if err == nil {
		err = closeErr
	}
	if err != nil {
		_ = os.Remove(name)
	}
	return err
}

func validateGuardedPlan(plan Plan) error {
	violations := ValidatePlanAll(plan)
	if len(violations) == 0 {
		return nil
	}
	messages := make([]string, len(violations))
	for i, v := range violations {
		messages[i] = v.Message
	}
	return errors.New(strings.Join(messages, "\n"))
}

// readGuardedInput reads plan, diff, or scratch input. "-" selects stdin, and a
// non-regular file is rejected so a directory or device is never read as text.
func readGuardedInput(name string) ([]byte, error) {
	var reader io.Reader = os.Stdin
	if name != "-" {
		f, err := os.Open(name)
		if err != nil {
			return nil, err
		}
		defer func() { _ = f.Close() }()
		info, err := f.Stat()
		if err != nil {
			return nil, err
		}
		if !info.Mode().IsRegular() {
			return nil, fmt.Errorf("expected regular file: %s", name)
		}
		reader = f
	}
	raw, err := io.ReadAll(io.LimitReader(reader, planpatch.MaxPayload+1))
	if err == nil && len(raw) > planpatch.MaxPayload {
		err = fmt.Errorf("input exceeds %d bytes", planpatch.MaxPayload)
	}
	return raw, err
}
