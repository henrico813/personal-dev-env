package internal

import (
	"bytes"
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
  planner template --md
  planner new <output.md> [--diff] [--dry-run] [--json-errors]
  planner template --json [--section <s> [--subsection <x>] [--file <filename>] [--field <field>]]
  planner template --raw --section <s> [--subsection <x>] [--file <filename>] [--field <field>]
  planner template --help
  planner check [<plan.md|plan.json>] [--format md|json] [--stdin] [--json-errors]  Reports every violation in one run.
  planner create [<plan.json>] <output.md> [--stdin] [--diff] [--dry-run] [--json-errors]
  planner inspect <plan.md>
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

Legacy JSON render flow:
  1. Run planner template --json > draft.json.
  2. Edit the draft JSON directly.
  3. Run planner check draft.json.
  4. Run planner create draft.json out.md.

Rewrite flow (full rewrite):
  1. Read the existing markdown issue.
  2. Map its content into canonical JSON matching planner template --json.
  3. Run planner check <plan.json>.
  4. Run planner create <plan.json> <output.md>.
  5. Compare the rendered issue with the source issue for dropped content.

Partial update flow:
  1. Run planner inspect <plan.md> to see the parsed plan JSON.
  2. Use behavioral commands such as planner title set, planner dod goal set,
     planner implementation step file-change diff set, and planner verification automated add.
  3. Non-targeted sections remain byte-for-byte unchanged.

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

template selectors:
  --md                             Print the canonical markdown plan with PLACEHOLDER text.
  --json                           Print the full JSON skeleton.
  --json --section <s>             Print a section-level JSON shape.
  --json --section <s> --subsection <x>  Print a subsection-level JSON shape.
  --json --section implementation --subsection N --field <field>  Print a leaf shape.
  --help                           Walk through the create workflow and PLACEHOLDER convention.
  Note: --section without --json or --raw is rejected with a USAGE error.

Validation rules:
`

const templateHelpText = `planner template -- print plan-shape references for AI authoring.

Usage:
  planner template --md
  planner template --json
  planner template --raw --section <s> [--subsection <x>] [--file <filename>] [--field <field>]
  planner template --json --section <s> [--subsection <x>] [--file <filename>] [--field <field>]

Selectors:
  --md                  Canonical markdown plan with PLACEHOLDER text and validation hints.
  --json                Full plan JSON skeleton; FileChange.Diff is the literal string "PLACEHOLDER".
  --section/-s <s>      Section-level JSON shape: title, overview, definition_of_done, implementation, verification.
  --subsection <x>      Field name for definition_of_done; 1-based step index for implementation; summary, automated, or manual for verification.
  --file <filename>     FileChange address helper for field-level selectors.
  --field <field>       Leaf selector: diff, title, summary, filename, explanation.
  --raw                 Emit raw text for scalar string selectors (no JSON quoting).
                        --field diff is the one selector that emits raw bytes and does not require --json.

Reference workflow:
  - planner new <output.md>
  - planner template --md
  - planner template --json or planner template --raw when you need a JSON skeleton or scalar selector
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
	case "template":
		return runTemplate(args[1:], stdout, stderr)
	case "new":
		return runNew(args[1:], stdout, stderr)
	case "check":
		return runCheck("check", args[1:], stdout, stderr)
	case "create":
		return runCreate(args[1:], stdout, stderr)
	case "inspect":
		return runInspect(args[1:], stdout, stderr)
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

type templateOptions struct {
	md         bool
	jsonMode   bool
	rawMode    bool
	section    string
	subsection string
	file       string
	field      string
}

func parseTemplateOptions(args []string) (templateOptions, error) {
	opts := templateOptions{}
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--md":
			opts.md = true
		case "--json":
			opts.jsonMode = true
		case "--section", "-s":
			i++
			if i >= len(args) {
				return opts, fmt.Errorf("missing value for --section")
			}
			opts.section = args[i]
		case "--subsection":
			i++
			if i >= len(args) {
				return opts, fmt.Errorf("missing value for --subsection")
			}
			opts.subsection = args[i]
		case "--file":
			i++
			if i >= len(args) {
				return opts, fmt.Errorf("missing value for --file")
			}
			opts.file = args[i]
		case "--field":
			i++
			if i >= len(args) {
				return opts, fmt.Errorf("missing value for --field")
			}
			opts.field = args[i]
		case "--raw":
			opts.rawMode = true
		default:
			return opts, fmt.Errorf("unknown flag %q", args[i])
		}
	}
	if opts.md && opts.jsonMode {
		return opts, fmt.Errorf("--md and --json are mutually exclusive")
	}
	if opts.rawMode && (opts.md || opts.jsonMode) {
		return opts, fmt.Errorf("--raw is mutually exclusive with --md and --json")
	}
	if opts.md && (opts.section != "" || opts.subsection != "" || opts.file != "" || opts.field != "") {
		return opts, fmt.Errorf("--md does not accept selectors")
	}
	if opts.rawMode && opts.section == "" {
		return opts, fmt.Errorf("--raw requires --section")
	}
	if opts.subsection != "" && opts.section == "" {
		return opts, fmt.Errorf("--subsection requires --section")
	}
	if opts.section != "" && !opts.jsonMode && !opts.rawMode && opts.field != "diff" {
		return opts, fmt.Errorf("--section requires --json")
	}
	if !opts.md && !opts.jsonMode && !opts.rawMode && opts.field != "diff" {
		return opts, fmt.Errorf("either --md or --json is required")
	}
	return opts, nil
}

// validateFieldGrammar is the shared leaf-selector validator for patch and
// template. It keeps both commands aligned on the same section/subsection/file
// and field combinations.
func validateFieldGrammar(opts ReplaceOptions) error {
	if opts.Append && opts.Section != "implementation" {
		return fmt.Errorf("--append is only valid with --section implementation")
	}
	if opts.Append && opts.Subsection != "" {
		return fmt.Errorf("--append and --subsection cannot be used together")
	}
	if opts.Append && opts.Field != "" {
		return fmt.Errorf("--append cannot be used with --field")
	}
	if opts.Section == "title" {
		if opts.Subsection != "" || opts.File != "" || opts.Field != "" || opts.Append {
			return fmt.Errorf("--section title accepts no other selectors")
		}
		return nil
	}
	if opts.Section == "verification" && opts.Subsection != "" {
		switch opts.Subsection {
		case "summary", "automated", "manual":
		default:
			return fmt.Errorf("invalid verification subsection %q: valid values are summary, automated, manual", opts.Subsection)
		}
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
			if opts.File == "" {
				return fmt.Errorf("--field %s requires --file F", opts.Field)
			}
		case "title", "summary":
			if opts.File != "" {
				return fmt.Errorf("--field %s does not take --file", opts.Field)
			}
		default:
			return fmt.Errorf("--field %q not valid (allowed: diff, title, summary, filename, explanation)", opts.Field)
		}
	}
	if opts.File != "" && opts.Field == "" {
		return fmt.Errorf("--file requires --field")
	}
	return nil
}

func runTemplate(args []string, stdout io.Writer, stderr io.Writer) int {
	for _, a := range args {
		if a == "--help" || a == "-h" {
			_, _ = io.WriteString(stdout, templateHelpText)
			return 0
		}
	}

	opts, err := parseTemplateOptions(args)
	if err != nil {
		reportError(stderr, "template", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if err := validateFieldGrammar(ReplaceOptions{
		Section:    opts.section,
		Subsection: opts.subsection,
		File:       opts.file,
		Field:      opts.field,
	}); err != nil {
		reportError(stderr, "template", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if opts.rawMode {
		if err := validateTemplateRawMode(opts); err != nil {
			reportError(stderr, "template", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
	}

	if opts.md {
		rendered, err := renderCanonicalScaffold()
		if err != nil {
			reportError(stderr, "template", newPlannerCLIError(PlannerRenderOutputError, err, "plan markdown"))
			return 1
		}
		_, _ = io.WriteString(stdout, rendered)
		return 0
	}

	plan := BuildPlanTemplate()
	switch {
	case opts.section == "":
		raw, err := MarshalJSONNoEscape(plan)
		if err != nil {
			reportError(stderr, "template", newPlannerCLIError(PlannerRenderOutputError, err, "template JSON"))
			return 1
		}
		_, _ = stdout.Write(append(raw, '\n'))
		return 0
	case opts.rawMode:
		raw, err := scalarTemplateValue(plan, opts)
		if err != nil {
			reportError(stderr, "template", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		_, _ = io.WriteString(stdout, raw+"\n")
		return 0
	case opts.field == "diff":
		_, _ = stdout.Write([]byte("--- a/<path>\n+++ b/<path>\n@@ -1 +1 @@\n-old\n+new\n"))
		return 0
	default:
		raw, err := MarshalSection(plan, opts.section, opts.subsection, opts.file, opts.field)
		if err != nil {
			reportError(stderr, "template", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		_, _ = stdout.Write(raw)
		return 0
	}
}

func validateTemplateRawMode(opts templateOptions) error {
	if !isScalarTemplate(opts) {
		return fmt.Errorf("--raw is only valid for scalar string selectors")
	}
	return nil
}

func isScalarTemplate(opts templateOptions) bool {
	switch opts.section {
	case "title", "overview":
		return true
	case "definition_of_done":
		return opts.subsection == "narrative" || opts.subsection == "current_state" || opts.subsection == "module_shape"
	case "implementation":
		switch opts.field {
		case "title", "summary", "filename", "explanation":
			return true
		}
	case "verification":
		return opts.subsection == "summary"
	}
	return false
}

func scalarTemplateValue(plan Plan, opts templateOptions) (string, error) {
	raw, err := MarshalSection(plan, opts.section, opts.subsection, opts.file, opts.field)
	if err != nil {
		return "", err
	}
	var value string
	if err := json.Unmarshal(raw, &value); err != nil {
		return "", err
	}
	return value, nil
}

// detectFormat infers the plan format from a filename extension.
func detectFormat(path string) string {
	lower := strings.ToLower(path)
	switch {
	case strings.HasSuffix(lower, ".md"):
		return "md"
	case strings.HasSuffix(lower, ".json"):
		return "json"
	default:
		return ""
	}
}

// runCheck validates markdown or JSON plans and reports every violation.
func runCheck(cmd string, args []string, stdout io.Writer, stderr io.Writer) int {
	format := ""
	rest := make([]string, 0, len(args))
	for i := 0; i < len(args); i++ {
		if args[i] == "--format" {
			i++
			if i >= len(args) {
				reportError(stderr, cmd, newPlannerCLIError(PlannerUsageError, nil, "missing value for --format"))
				return 2
			}
			format = args[i]
			continue
		}
		rest = append(rest, args[i])
	}

	positional, pf, err := splitPreviewArgs(rest, false, true)
	if err != nil {
		reportError(stderr, cmd, newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if (len(positional) == 0 && !pf.stdin) || len(positional) > 1 {
		reportError(stderr, cmd, newPlannerCLIError(PlannerUsageError, nil, "usage: planner check [<plan.md|plan.json>] [--format md|json] [--stdin]"))
		return 2
	}

	path := ""
	if len(positional) == 1 {
		path = positional[0]
	}
	if format == "" && path != "" {
		format = detectFormat(path)
	}
	if format == "" {
		reportError(stderr, cmd, newPlannerCLIError(PlannerUsageError, nil, "--format md|json is required for stdin or paths with no recognised extension"))
		return 2
	}
	if format != "md" && format != "json" {
		reportError(stderr, cmd, newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--format %q is not valid; use md or json", format)))
		return 2
	}

	var plan Plan
	if format == "md" {
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
			reportError(stderr, cmd, newPlannerCLIError(PlannerDecodeInputError, parseErr, "plan markdown"))
			return 1
		}
		plan = parsed.Plan
	} else {
		plan, err = readPlanFrom(filterNonEmpty([]string{path}), pf.stdin, stderr)
		if err != nil {
			reportError(stderr, cmd, err)
			return plannerExitCode(err)
		}
	}

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

func runCreate(args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) >= 1 && args[0] == "step" {
		reportError(stderr, "create", newPlannerCLIError(PlannerUsageError, nil, "planner create step is no longer supported; rewrite the full plan JSON and run planner create <plan.json> <output.md>"))
		return 2
	}
	positional, pf, err := splitPreviewArgs(args, true, true)
	if err != nil {
		reportError(stderr, "create", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	var inputPath, outputPath string
	switch len(positional) {
	case 2:
		inputPath, outputPath = positional[0], positional[1]
	case 1:
		outputPath = positional[0]
	default:
		reportError(stderr, "create", newPlannerCLIError(PlannerUsageError, nil, "usage: planner create [<plan.json>] <output.md> [--stdin] [--diff] [--dry-run]"))
		return 2
	}
	plan, err := readPlanFrom(filterNonEmpty([]string{inputPath}), pf.stdin, stderr)
	if err != nil {
		reportError(stderr, "create", err)
		return plannerExitCode(err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		reportError(stderr, "create", newPlannerCLIError(PlannerRenderOutputError, err, "plan markdown"))
		return 1
	}
	if err := ValidatePlan(plan); err != nil {
		reportError(stderr, "create", newPlannerCLIError(PlannerValidateInputError, err, "plan"))
		return 1
	}
	if err := VerifyRenderedText(rendered, plan); err != nil {
		reportError(stderr, "create", newPlannerCLIError(PlannerValidateInputError, err, "rendered plan"))
		return 1
	}
	return runPreview(stdout, stderr, pf, rendered, outputPath, "create", func() error {
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
		reportError(stderr, "inspect", newPlannerCLIError(PlannerDecodeInputError, err, "plan markdown"))
		return 1
	}

	out, err := json.MarshalIndent(parsed.Plan, "", "  ")
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

// readJSONSource returns JSON bytes for a subcommand's input and reports
// whether repair produced replacement bytes. When --stdin is set, reads stdin.
// When allowAutoDetect is true (validate/create only) and no path is supplied
// and stdin is piped, reads stdin. Otherwise reads the path.
func readJSONSource(path string, useStdin, allowAutoDetect bool, stderr io.Writer) ([]byte, bool, error) {
	if useStdin && !stdinPiped() && !jsonErrorOutput {
		_, _ = fmt.Fprintln(stderr, "planner: reading JSON from stdin (Ctrl-D to end)")
	}
	data, repaired, err := Read(path, useStdin, allowAutoDetect, os.Stdin, stdinPiped)
	if err != nil {
		return nil, false, err
	}
	if repaired && !jsonErrorOutput {
		_, _ = fmt.Fprintln(stderr, "planner: repaired JSON input")
	}
	return data, repaired, nil
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

// readPlanFrom is the plan-decoding wrapper for runCheck/runCreate.
// Decode errors are wrapped in typed planner CLI errors so tests can assert on
// stable failure categories instead of raw strings.
func readPlanFrom(positional []string, useStdin bool, stderr io.Writer) (Plan, error) {
	path := ""
	if len(positional) > 0 {
		path = positional[0]
	}
	data, _, err := readJSONSource(path, useStdin, true, stderr)
	if err != nil {
		return Plan{}, newPlannerCLIError(PlannerReadInputError, err, patchSourceLabel(path, useStdin))
	}
	plan, err := DecodePlan(data)
	if err != nil {
		return Plan{}, newPlannerCLIError(PlannerDecodeInputError, err, "plan JSON")
	}
	return plan, nil
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
		return newPlannerCLIError(PlannerDecodeInputError, err, "plan markdown")
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
	default:
		return newPlannerCLIError(PlannerValidateInputError, err, "result")
	}
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

// filterNonEmpty drops empty-string entries, used to pass an optional input
// path to readPlanFrom without introducing an empty-string positional.
func filterNonEmpty(ss []string) []string {
	out := make([]string, 0, len(ss))
	for _, s := range ss {
		if s != "" {
			out = append(out, s)
		}
	}
	return out
}
