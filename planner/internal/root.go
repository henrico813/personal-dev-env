package internal

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"strings"

	"planner/internal/planpatch"
)

const helpText = `planner provides markdown-first implementation-plan workflows.

Usage:
  planner
  planner help
  planner new <output.md> [--diff] [--dry-run] [--json-errors]
  planner check <plan.md> [--repo DIR] [--base COMMIT] [--json-errors]
  planner inspect <plan.md>
  planner inspect <plan.md> --target SELECTOR --repo DIR --base COMMIT [--code-out NEWFILE [--before]] [--json-errors]
  planner patch <plan.md> --target SELECTOR --expect TOKEN --repo DIR --base COMMIT (--after-file FILE | --diff-file FILE) [--dry-run] [--diff] [--json-errors]

Global flags:
  --json-errors                    Emit failures as structured JSON to stderr ({code, message, recovery_hint?}).

Markdown-first authoring:
  1. Run planner new plan.md. It fails without changing an existing destination.
  2. Edit prose and structure directly in the Markdown file.
  3. Change code diffs only through guarded patch: inspect with --code-out to
     export the source and read edit_expect, edit that source, then patch with
     --after-file.
  4. Add or remove a file change by hand: copy a PLACEHOLDER fence, fill it with
     inspect --before and patch, then delete the old block. A step keeps at
     least one file change.
  5. Finish with planner check plan.md as the final gate. It reports every
     structure violation and applies every diff at the baseline.
`

const validationRulesHeader = "\nValidation rules:\n"

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
		return runGuardedCheck(args[1:], stdout, stderr)
	case "inspect":
		if hasArg(args[1:], "--target") {
			return runGuardedInspect(args[1:], stdout, stderr)
		}
		return runInspect(args[1:], stdout, stderr)
	case "patch":
		return runGuardedPatch(args[1:], stdout, stderr)
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
	if _, err := os.Lstat(outputPath); err == nil {
		reportError(stderr, "new", newPlannerCLIError(PlannerWriteOutputError, os.ErrExist, outputPath))
		return 1
	} else if !errors.Is(err, os.ErrNotExist) {
		reportError(stderr, "new", newPlannerCLIError(PlannerReadInputError, err, outputPath))
		return 1
	}
	rendered, err := renderCanonicalScaffold()
	if err != nil {
		reportError(stderr, "new", newPlannerCLIError(PlannerRenderOutputError, err, "plan markdown"))
		return 1
	}
	return runPreview(stdout, stderr, pf, rendered, outputPath, "new", func() error {
		if err := planpatch.WriteNew(outputPath, []byte(rendered)); err != nil {
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
	b.WriteString(guardedHelp)
	b.WriteString(validationRulesHeader)
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
