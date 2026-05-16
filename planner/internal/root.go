package internal

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"strings"
)

const helpText = `planner provides markdown-first implementation-plan workflows.

Usage:
  planner
  planner help
  planner new <output.md> [--diff] [--dry-run] [--json-errors]
  planner check [<plan.md>] [--stdin] [--json-errors]  Reports every violation in one run.
  planner inspect <plan.md>
  planner patch <plan.md> [<out.md>]
  planner title set <plan.md> <out.md> [<text>] [--stdin] [--diff] [--dry-run] [--json-errors]
  planner overview set <plan.md> <out.md> [<text>] [--stdin] [--diff] [--dry-run] [--json-errors]
  planner dod narrative set <plan.md> <out.md> [<text>] [--stdin] [--diff] [--dry-run] [--json-errors]
  planner dod current-state set <plan.md> <out.md> [<text>] [--stdin] [--diff] [--dry-run] [--json-errors]
  planner dod module-shape set <plan.md> <out.md> [<text>] [--stdin] [--diff] [--dry-run] [--json-errors]
  planner dod goal add <plan.md> <out.md> <text> [--diff] [--dry-run] [--json-errors]
  planner dod goal set <plan.md> <out.md> --goal N <text> [--diff] [--dry-run] [--json-errors]
  planner dod goal remove <plan.md> <out.md> --goal N [--diff] [--dry-run] [--json-errors]
  planner implementation step add <plan.md> <out.md> --title T --summary S --filename F --explanation E --diff-stdin [--diff] [--dry-run] [--json-errors]
  planner implementation step remove <plan.md> <out.md> --step N [--diff] [--dry-run] [--json-errors]
  planner implementation step title set <plan.md> <out.md> --step N [<text>] [--stdin] [--diff] [--dry-run] [--json-errors]
  planner implementation step summary set <plan.md> <out.md> --step N [<text>] [--stdin] [--diff] [--dry-run] [--json-errors]
  planner implementation step file-change add <plan.md> <out.md> --step N --filename F --explanation E --diff-stdin [--diff] [--dry-run] [--json-errors]
  planner implementation step file-change remove <plan.md> <out.md> --step N --change N [--diff] [--dry-run] [--json-errors]
  planner implementation step file-change filename set <plan.md> <out.md> --step N --change N [<text>] [--stdin] [--diff] [--dry-run] [--json-errors]
  planner implementation step file-change explanation set <plan.md> <out.md> --step N --change N [<text>] [--stdin] [--diff] [--dry-run] [--json-errors]
  planner implementation step file-change diff set <plan.md> <out.md> --step N --change N --stdin [--diff] [--dry-run] [--json-errors]
  planner verification summary set <plan.md> <out.md> [<text>] [--stdin] [--diff] [--dry-run] [--json-errors]
  planner verification automated add <plan.md> <out.md> <text> [--diff] [--dry-run] [--json-errors]
  planner verification automated set <plan.md> <out.md> --item N <text> [--diff] [--dry-run] [--json-errors]
  planner verification automated remove <plan.md> <out.md> --item N [--diff] [--dry-run] [--json-errors]
  planner verification manual add <plan.md> <out.md> <text> [--diff] [--dry-run] [--json-errors]
  planner verification manual set <plan.md> <out.md> --item N <text> [--diff] [--dry-run] [--json-errors]
  planner verification manual remove <plan.md> <out.md> --item N [--diff] [--dry-run] [--json-errors]

Global flags:
  --json-errors                    Emit failures as structured JSON to stderr ({code, message, recovery_hint?}).

Markdown-first authoring flow:
  1. Run planner new plan.md.
  2. Edit the markdown directly, or use behavioral edit commands for same-path updates.
  3. For behavioral edit commands, <out.md> may be the same path as <plan.md>
     for same-file updates.
  4. Run planner check plan.md --json-errors.
  5. If parsing fails, stop and escalate before rendering or applying more edits.

Partial update flow:
  1. Run planner inspect <plan.md> to see the parsed plan JSON and update_diff_expect tokens.
  2. Prefer planner patch <plan.md> [<out.md>] for scalar, checklist, and diff edits.
  3. planner patch preserves wrapped frontmatter but rerenders the body canonically.
  4. Use behavioral commands for structural edits that patch does not cover.

planner patch:
  Reads a structured patch from stdin and applies all operations to one plan.
  planner patch accepts no subcommand flags; only global flags such as
  --json-errors.

  Supported operations:
    *** Update Field: <selector>
    -<old line>
    +<new line>

    *** Update Diff: <selector>
    *** Expect: sha256:<token>
    <raw diff body through EOF>

    *** Add Item: <selector>
    +<new checklist item>

  Supported field selectors:
    title
    overview
    definition_of_done.narrative
    definition_of_done.current_state
    definition_of_done.module_shape
    implementation[N].title
    implementation[N].summary
    implementation[N].file_changes[N].filename
    implementation[N].file_changes[N].explanation
    verification.summary

  Supported checklist selectors:
    definition_of_done.goals
    verification.automated
    verification.manual

  Supported diff selectors:
    implementation[N].file_changes[N]

  Notes:
    - Nested selectors use 1-based indices.
    - Patch body lines beginning with *** start the next patch operation.
    - Update Diff is a dedicated single-op patch form.
    - Update Diff tokens come from planner inspect.
    - Checklist edits must be single-line.
    - Only verification.summary may be set to an empty value.
    - Structural edits should use behavioral commands.

behavioral edit flags:
  --goal N                         1-based definition_of_done goal selector.
  --item N                         1-based verification checklist selector.
  --step N                         1-based implementation step selector.
  --change N                       1-based FileChange selector within --step.
  --filename <path>                FileChange filename for structured add.
  --explanation <text>             FileChange explanation for structured add.
  --stdin                          Read scalar values or file-change diff set from stdin.
  --diff-stdin                     Read structured add diff body from stdin.
  --diff                           Print preview diff to stdout; additive.
  --dry-run                        Do not write the output; with --diff, exit 1 on drift.

Validation rules:
`

var jsonErrorOutput bool

// Execute is the production command entrypoint used by main() and CLI tests.
func Execute(args []string, stdout io.Writer, stderr io.Writer) int {
	args, jsonErrorOutput = extractJSONErrorsFlag(args)
	defer func() { jsonErrorOutput = false }()

	if len(args) == 0 {
		printHelp(stdout)
		return 0
	}

	switch args[0] {
	case "help", "--help", "-h":
		printHelp(stdout)
		return 0
	case "new":
		return runNew(args[1:], stdout, stderr)
	case "check":
		return runCheck("check", args[1:], stdout, stderr)
	case "inspect":
		return runInspect(args[1:], stdout, stderr)
	case "patch":
		return runPatch(args[1:], stdout, stderr)
	case "title", "overview", "dod", "implementation", "verification":
		return runBehavioralEdit(args, stdout, stderr)
	default:
		reportError(stderr, "planner", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("unknown command: %s", args[0])))
		// Help text is verbose human-oriented prose; under --json-errors the
		// stderr stream must stay machine-parseable, so suppress the dump.
		if !jsonErrorOutput {
			printHelp(stderr)
		}
		return 2
	}
}

func extractJSONErrorsFlag(args []string) ([]string, bool) {
	kept := make([]string, 0, len(args))
	found := false
	for _, arg := range args {
		if arg == "--json-errors" {
			found = true
			continue
		}
		kept = append(kept, arg)
	}
	return kept, found
}

func reportError(stderr io.Writer, cmd string, err error) {
	if err == nil {
		return
	}
	var cliErr *PlannerCLIError
	if !errors.As(err, &cliErr) {
		// Untyped errors are runtime failures. Misclassifying them as
		// validation errors would lie to AIs branching on the JSON code, so
		// the fallback is RUNTIME and call sites are expected to construct
		// typed errors directly when origin is known.
		cliErr = newPlannerCLIError(PlannerRuntimeError, err, err.Error())
	}
	if jsonErrorOutput {
		raw, marshalErr := json.Marshal(cliErr)
		if marshalErr != nil {
			_, _ = fmt.Fprintf(stderr, "%s: %v\n", cmd, marshalErr)
			return
		}
		_, _ = fmt.Fprintln(stderr, string(raw))
		return
	}
	_, _ = fmt.Fprintf(stderr, "%s: %v\n", cmd, cliErr)
}

func plannerMarkdownDecodeError(raw []byte, parseErr error) *PlannerCLIError {
	subject := "plan markdown"
	wrapped, malformedWrapper := wrappedDocContext(parseErr)
	if wrapped {
		subject = "wrapped issue doc markdown"
	}
	cliErr := newPlannerCLIError(PlannerDecodeInputError, parseErr, subject)
	if malformedWrapper {
		cliErr.RecoveryHint = "use the supported vault issue frontmatter block or remove the wrapper before retrying"
	}
	return cliErr
}

// runCheck validates markdown plans and reports every violation.
func runCheck(cmd string, args []string, stdout io.Writer, stderr io.Writer) int {
	const usage = "usage: planner check [<plan.md>] [--stdin] [--json-errors]"
	for _, a := range args {
		if a == "--format" {
			reportError(stderr, cmd, newPlannerCLIError(PlannerUsageError, nil, usage))
			return 2
		}
	}
	positional, pf, err := splitPreviewArgs(args, false, true)
	if err != nil {
		reportError(stderr, cmd, newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if (len(positional) == 0 && !pf.stdin) || len(positional) > 1 {
		reportError(stderr, cmd, newPlannerCLIError(PlannerUsageError, nil, usage))
		return 2
	}

	path := ""
	if len(positional) == 1 {
		path = positional[0]
	}
	if path != "" && strings.HasSuffix(strings.ToLower(path), ".json") {
		reportError(stderr, cmd, newPlannerCLIError(PlannerUsageError, nil, "planner check no longer accepts JSON plan input: "+usage))
		return 2
	}

	var raw []byte
	if pf.stdin {
		raw, err = io.ReadAll(os.Stdin)
	} else {
		raw, err = os.ReadFile(path)
	}
	if err != nil {
		reportError(stderr, cmd, newPlannerCLIError(PlannerReadInputError, err, patchSourceLabel(path, pf.stdin)))
		return 1
	}
	parsed, parseErr := ParseMarkdown(string(raw))
	if parseErr != nil {
		reportError(stderr, cmd, plannerMarkdownDecodeError(raw, parseErr))
		return 1
	}
	plan := parsed.Plan

	if errs := ValidatePlanAll(plan); len(errs) > 0 {
		messages := make([]string, len(errs))
		for i, e := range errs {
			messages[i] = e.Message
		}
		reportError(stderr, cmd, newPlannerCLIError(PlannerValidateInputError, errors.New(strings.Join(messages, "\n")), "plan"))
		return 1
	}
	_, _ = io.WriteString(stdout, "OK\n")
	return 0
}

func runNew(args []string, stdout io.Writer, stderr io.Writer) int {
	const usage = "usage: planner new <output.md> [--diff] [--dry-run] [--json-errors]"
	const nonMarkdownUsage = "planner new requires an output path ending in .md: " + usage

	positional, pf, err := splitPreviewArgs(args, true, false)
	if err != nil {
		reportError(stderr, "new", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if len(positional) != 1 {
		reportError(stderr, "new", newPlannerCLIError(PlannerUsageError, nil, usage))
		return 2
	}
	outputPath := positional[0]
	if !strings.HasSuffix(strings.ToLower(outputPath), ".md") {
		reportError(stderr, "new", newPlannerCLIError(PlannerUsageError, nil, nonMarkdownUsage))
		return 2
	}
	rendered, err := renderCanonicalScaffold()
	if err != nil {
		reportError(stderr, "new", newPlannerCLIError(PlannerRenderOutputError, err, "plan markdown"))
		return 1
	}
	return runPreview(stdout, stderr, pf, rendered, outputPath, "new", func() error {
		if err := WriteAtomic(outputPath, []byte(rendered)); err != nil {
			return newPlannerCLIError(PlannerWriteOutputError, err, outputPath)
		}
		return nil
	}, outputPath)
}

func printHelp(w io.Writer) {
	_, _ = io.WriteString(w, buildHelpText())
}

func buildHelpText() string {
	var b strings.Builder
	b.WriteString(helpText)
	for _, rule := range ValidationRules() {
		b.WriteString("  - " + rule + "\n")
	}
	return b.String()
}

func runInspect(args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) != 1 {
		reportError(stderr, "inspect", newPlannerCLIError(PlannerUsageError, nil, "usage: planner inspect <plan.md>"))
		return 2
	}
	raw, err := os.ReadFile(args[0])
	if err != nil {
		reportError(stderr, "inspect", newPlannerCLIError(PlannerReadInputError, err, args[0]))
		return 1
	}

	parsed, err := ParseMarkdown(string(raw))
	if err != nil {
		reportError(stderr, "inspect", plannerMarkdownDecodeError(raw, err))
		return 1
	}

	view := buildInspectPlan(parsed, string(raw))
	out, err := json.MarshalIndent(view, "", "  ")
	if err != nil {
		reportError(stderr, "inspect", newPlannerCLIError(PlannerWriteOutputError, err, "inspect JSON"))
		return 1
	}
	_, _ = stdout.Write(append(out, '\n'))
	return 0
}

func runPatchPlaceholder(args []string, stdout io.Writer, stderr io.Writer) int {
	const usage = "usage: planner patch <plan.md> [<out.md>]"
	reportError(stderr, "patch", newPlannerCLIError(PlannerUsageError, nil, usage))
	return 2
}

// previewFlags carries the preview-state flags stripped before subcommand
// parsing. Write is the default; --dry-run opts out of it.
type previewFlags struct {
	stdin  bool
	diff   bool
	dryRun bool
}

// splitPreviewArgs separates --stdin/--diff/--dry-run from positional and
// subcommand flags. When allowPreview is false, --diff/--dry-run are passed
// through unchanged (reject at the subcommand layer). When allowStdin is
// false, --stdin is also passed through.
func splitPreviewArgs(args []string, allowPreview, allowStdin bool) ([]string, previewFlags, error) {
	kept := []string{}
	pf := previewFlags{}
	for _, a := range args {
		switch {
		case a == "--stdin" && allowStdin:
			pf.stdin = true
		case a == "--diff" && allowPreview:
			pf.diff = true
		case a == "--dry-run" && allowPreview:
			pf.dryRun = true
		case a == "--write":
			return nil, pf, fmt.Errorf("unknown flag %q", a)
		default:
			kept = append(kept, a)
		}
	}
	return kept, pf, nil
}

// readRawSource reads patch input as raw bytes without JSON repair. It mirrors
// readJSONSource's stdin/path selection but preserves the byte stream exactly.
func readRawSource(path string, useStdin bool) ([]byte, error) {
	if useStdin {
		return io.ReadAll(os.Stdin)
	}
	if path == "" {
		return nil, fmt.Errorf("no patch path and --stdin not set")
	}
	return os.ReadFile(path)
}

// readRawScalar reads raw patch input and strips exactly one trailing line
// ending before the value is handed to scalar string patch targets.
func readRawScalar(path string, useStdin bool) ([]byte, error) {
	data, err := readRawSource(path, useStdin)
	if err != nil {
		return nil, err
	}
	if bytes.HasSuffix(data, []byte("\r\n")) {
		return data[:len(data)-2], nil
	}
	if bytes.HasSuffix(data, []byte("\n")) {
		return data[:len(data)-1], nil
	}
	return data, nil
}

func patchSourceLabel(path string, useStdin bool) string {
	if useStdin {
		return "stdin"
	}
	if path == "" {
		return "JSON input"
	}
	return path
}

// mapReplaceCLIError translates internal replace package failures into CLI
// envelopes. Subject strings describe data ("result", "patch JSON"), not the
// command name; the cmd argument to reportError owns the CLI label.
func mapReplaceCLIError(err error, sourcePath string) *PlannerCLIError {
	var replaceErr *ReplaceError
	if !errors.As(err, &replaceErr) {
		return newPlannerCLIError(PlannerValidateInputError, err, "result")
	}
	switch replaceErr.Code {
	case ReplaceInvalidOptionsError:
		return newPlannerCLIError(PlannerUsageError, err, err.Error())
	case ReplaceReadSourceError:
		return newPlannerCLIError(PlannerReadInputError, err, sourcePath)
	case ReplaceParseSourceError:
		return plannerMarkdownDecodeError(nil, err)
	case ReplaceDecodePatchError:
		return newPlannerCLIError(PlannerDecodeInputError, err, "patch JSON")
	case ReplaceRenderResultError:
		return newPlannerCLIError(PlannerRenderOutputError, err, "updated plan markdown")
	case ReplaceValidateResultError:
		return newPlannerCLIError(PlannerValidateInputError, err, "updated plan")
	case ReplaceFileNotFoundError:
		e := newPlannerCLIError(PlannerUsageError, err, err.Error())
		e.RecoveryHint = "run planner inspect <plan.md> to list valid filenames in the targeted step"
		return e
	case ReplaceFileAmbiguousError:
		e := newPlannerCLIError(PlannerUsageError, err, err.Error())
		e.RecoveryHint = "rename or consolidate duplicate FileChange filenames before patching"
		return e
	case ReplaceParseSplicedSourceError:
		e := newPlannerCLIError(PlannerValidateInputError, err, "spliced plan markdown")
		e.RecoveryHint = "remove or escape triple-backtick fences in the diff body, then patch again"
		return e
	case ReplacePatchSyntaxError:
		return newPlannerCLIError(PlannerDecodeInputError, err, "structured patch")
	case ReplacePatchSelectorError:
		e := newPlannerCLIError(PlannerValidateInputError, err, "structured patch")
		e.RecoveryHint = "use the documented selector grammar or fall back to a behavioral command"
		return e
	case ReplacePatchMismatchError:
		e := newPlannerCLIError(PlannerValidateInputError, err, "patch old value")
		e.RecoveryHint = "refresh the old value from planner inspect or the current file, then retry"
		return e
	case ReplacePatchExpectMismatchError:
		e := newPlannerCLIError(PlannerValidateInputError, err, "patch diff token")
		e.RecoveryHint = "refresh update_diff_expect from planner inspect, then retry"
		return e
	default:
		return newPlannerCLIError(PlannerValidateInputError, err, "result")
	}
}

type InspectPlan struct {
	Title            string           `json:"title"`
	Overview         string           `json:"overview"`
	DefinitionOfDone DefinitionOfDone `json:"definition_of_done"`
	Implementation   []InspectStep    `json:"implementation"`
	Verification     *Verification    `json:"verification"`
}

type InspectStep struct {
	Title       string              `json:"title"`
	Summary     string              `json:"summary"`
	FileChanges []InspectFileChange `json:"file_changes"`
}

type InspectFileChange struct {
	Filename         string `json:"filename"`
	Explanation      string `json:"explanation"`
	Diff             string `json:"diff"`
	Selector         string `json:"selector"`
	UpdateDiffExpect string `json:"update_diff_expect"`
}

func buildInspectPlan(parsed ParseResult, source string) InspectPlan {
	view := InspectPlan{
		Title:            parsed.Plan.Title,
		Overview:         parsed.Plan.Overview,
		DefinitionOfDone: parsed.Plan.DefinitionOfDone,
		Implementation:   make([]InspectStep, 0, len(parsed.Plan.Implementation)),
		Verification:     parsed.Plan.Verification,
	}
	for stepIdx, step := range parsed.Plan.Implementation {
		inspectStep := InspectStep{Title: step.Title, Summary: step.Summary}
		for changeIdx, change := range step.FileChanges {
			raw := rawAt(source, parsed.DiffContents[stepIdx][changeIdx])
			selector := fmt.Sprintf("implementation[%d].file_changes[%d]", stepIdx+1, changeIdx+1)
			inspectStep.FileChanges = append(inspectStep.FileChanges, InspectFileChange{
				Filename:         change.Filename,
				Explanation:      change.Explanation,
				Diff:             change.Diff,
				Selector:         selector,
				UpdateDiffExpect: buildUpdateDiffExpect(selector, change.Filename, change.Explanation, raw),
			})
		}
		view.Implementation = append(view.Implementation, inspectStep)
	}
	return view
}

func buildUpdateDiffExpect(selector, filename, explanation, diffRaw string) string {
	h := sha256.New()
	_, _ = io.WriteString(h, selector)
	_, _ = io.WriteString(h, "\x00")
	_, _ = io.WriteString(h, filename)
	_, _ = io.WriteString(h, "\x00")
	_, _ = io.WriteString(h, explanation)
	_, _ = io.WriteString(h, "\x00")
	_, _ = io.WriteString(h, diffRaw)
	return "sha256:" + hex.EncodeToString(h.Sum(nil))
}

// stdinPiped reports whether os.Stdin has piped data (not a terminal).
func stdinPiped() bool {
	fi, err := os.Stdin.Stat()
	return err == nil && (fi.Mode()&os.ModeCharDevice) == 0
}

// runPreview orchestrates the create preview flow. Write is the default;
// --dry-run suppresses it. --diff is additive and still writes unless dry-run
// is set. stdoutPathOnWrite is printed on successful writes when --diff is not
// set, preserving the legacy "create prints the output path on success" stdout
// contract.
func runPreview(stdout, stderr io.Writer, pf previewFlags, rendered, basePath, cmdName string, doWrite func() error, stdoutPathOnWrite string) int {
	baseline, err := readBaseline(basePath)
	if err != nil {
		reportError(stderr, cmdName, newPlannerCLIError(PlannerReadInputError, err, basePath))
		return 1
	}
	d := diffLines(baseline, rendered)
	if pf.diff && d != "" {
		_, _ = io.WriteString(stdout, d)
	}
	if !pf.dryRun {
		if err := doWrite(); err != nil {
			reportError(stderr, cmdName, err)
			return 1
		}
		if !pf.diff && stdoutPathOnWrite != "" {
			_, _ = io.WriteString(stdout, stdoutPathOnWrite+"\n")
		}
		return 0
	}
	if pf.diff && d != "" {
		return 1
	}
	return 0
}

// runPreviewAgainstSource is runPreview with sourcePath as the baseline,
// used by patch so the diff shows what the patch changes in the source,
// not the difference from some unrelated output file.
func runPreviewAgainstSource(stdout, stderr io.Writer, pf previewFlags, rendered, sourcePath, outputPath, cmdName string, doWrite func() error) int {
	return runPreview(stdout, stderr, pf, rendered, sourcePath, cmdName, doWrite, "")
}

// readBaseline returns the existing file content for diff comparison. A
// missing file is equivalent to an empty baseline (new-file diff). Any other
// read error surfaces so permission-denied or EISDIR do not silently become
// empty baselines.
func readBaseline(path string) (string, error) {
	data, err := os.ReadFile(path)
	if errors.Is(err, os.ErrNotExist) {
		return "", nil
	}
	if err != nil {
		return "", err
	}
	return string(data), nil
}
