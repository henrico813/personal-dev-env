package main

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"strconv"
	"strings"

	"planner/inspect"
	"planner/internal/jsoninput"
	"planner/render"
	"planner/replace"
	"planner/schema"
	"planner/validate"
)

const helpText = `planner provides implementation-plan workflows from canonical JSON.

Usage:
  planner
  planner help
  planner template --md
  planner template --json [--section <s> [--subsection <x>] [--file <filename>] [--field <field>]]
  planner template --help
  planner validate [<plan.json>] [--stdin] [--json-errors]
  planner create [<plan.json>] <output.md> [--stdin] [--diff] [--dry-run] [--json-errors]
  planner inspect <plan.md>
  planner patch <plan.md> [<patch.json>|<diff.txt>] <output.md> --section <section> [--subsection <name-or-index>] [--file <filename>] [--field <field>] [--append] [--stdin] [--diff] [--dry-run] [--json-errors]

Global flags:
  --json-errors                    Emit failures as structured JSON to stderr ({code, message, recovery_hint?}).

Create flow:
  1. Research the task.
  2. Run planner template --json > draft.json (or planner template --help for the full walkthrough).
  3. Edit the draft JSON. Use planner patch --field diff for raw diff bodies.
  4. Run planner validate <plan.json>.
  5. Run planner create <plan.json> <output.md>.

Rewrite flow (full rewrite):
  1. Read the existing markdown issue.
  2. Map its content into canonical JSON matching planner template --json.
  3. Run planner validate <plan.json>.
  4. Run planner create <plan.json> <output.md>.
  5. Compare the rendered issue with the source issue for dropped content.

Partial update flow:
  1. Run planner inspect <plan.md> to see the parsed plan JSON.
  2. Run planner template --json --section <s> to learn the patch shape.
  3. Write patch JSON for the target scope.
  4. Run planner patch <plan.md> <patch.json> <output.md> --section <section>.
  5. Non-targeted sections remain byte-for-byte unchanged.

patch flags:
  --section/-s <section>           Required. One of: title, overview, definition_of_done, implementation, verification
  --subsection <name-or-index>     Optional. Field name for definition_of_done; 1-based step index for implementation; summary, automated, or manual for verification
  --file <filename>                Optional. With --field, addresses one FileChange inside an implementation step
  --field <field>                  Optional. One of: diff, title, summary, filename, explanation
  --append                         Optional. Append a new step to implementation
  --stdin                          Optional. Read patch JSON from stdin instead of a file
  --diff                           Optional. Print diff to stdout; additive (does not suppress write)
  --dry-run                        Optional. Do not write the output; with --diff, exit 1 on drift

template selectors:
  --md                             Print the canonical markdown plan with PLACEHOLDER text.
  --json                           Print the full JSON skeleton.
  --json --section <s>             Print a section-level JSON shape.
  --json --section <s> --subsection <x>  Print a subsection-level JSON shape.
  --json --section implementation --subsection N --field <field>  Print a leaf shape.
  --help                           Walk through the create workflow and PLACEHOLDER convention.
  Note: --section without --json is rejected with a USAGE error.

Validation rules:
`

const templateHelpText = `planner template -- print plan-shape references for AI authoring.

Usage:
  planner template --md
  planner template --json
  planner template --json --section <s> [--subsection <x>] [--file <filename>] [--field <field>]

Selectors:
  --md                  Canonical markdown plan with PLACEHOLDER text and validation hints.
  --json                Full plan JSON skeleton; FileChange.Diff is the literal string "PLACEHOLDER".
  --section/-s <s>      Section-level JSON shape: title, overview, definition_of_done, implementation, verification.
  --subsection <x>      Field name for definition_of_done; 1-based step index for implementation; summary, automated, or manual for verification.
  --file <filename>     FileChange address helper for field-level selectors.
  --field <field>       Leaf selector: diff, title, summary, filename, explanation.
                        --field diff is the one selector that emits raw bytes and does not require --json.

Create workflow:
  1. planner template --json > draft.json
  2. Edit fields. Use planner patch --field diff for raw unified diffs.
  3. planner validate draft.json && planner create draft.json out.md
`

const patchHelpText = `planner patch -- apply a patch to a section of an existing plan.

Usage:
  planner patch <plan.md> [<patch.json>|<diff.txt>] <output.md> --section <section> [--subsection <name-or-index>] [--file <filename>] [--field <field>] [--append] [--stdin] [--diff] [--dry-run]

Flags:
  --section/-s <section>           Required. One of: title, overview, definition_of_done, implementation, verification.
  --subsection <name-or-index>     Optional. Field name for definition_of_done; 1-based step index for implementation; summary, automated, or manual for verification.
  --file <filename>                Optional. With --field, addresses one FileChange inside an implementation step.
  --field <field>                  Optional. One of: diff, title, summary, filename, explanation.
  --append                         Optional. Append a new step to implementation.
  --stdin                          Optional. Read patch input from stdin instead of a file.
  --diff                           Optional. Print diff to stdout; additive (does not suppress write).
  --dry-run                        Optional. Do not write the output; with --diff, exit 1 on drift.

JSON-patch workflow:
  1. planner inspect <plan.md>
  2. planner template --json --section <s>
  3. Compose the patch JSON for the target scope.
  4. planner patch <plan.md> <patch.json> <output.md> --section <s>
  5. Non-targeted sections remain byte-for-byte unchanged.

Diff-edit workflow:
  1. planner inspect <plan.md>
  2. Find the implementation step number and FileChange filename.
  3. Write the new diff body as raw text.
  4. planner patch <plan.md> <diff.txt> <output.md> --section implementation --subsection N --file F --field diff
  5. Non-targeted sections remain byte-for-byte unchanged.

Field-edit workflow (Title):
  1. planner inspect <plan.md>
  2. Write a JSON string for the new title.
  3. planner patch <plan.md> <title.json> <output.md> --section title

Field-edit workflow (Step title or summary):
  1. planner inspect <plan.md>
  2. Write a JSON string for the new leaf.
  3. planner patch <plan.md> <leaf.json> <output.md> --section implementation --subsection N --field {title|summary}

Field-edit workflow (FileChange filename or explanation):
  1. planner inspect <plan.md>
  2. Write a JSON string for the new leaf.
  3. planner patch <plan.md> <leaf.json> <output.md> --section implementation --subsection N --file F --field {filename|explanation}

Field-edit workflow (Verification subsection):
  1. planner inspect <plan.md>
  2. Write a JSON string or checklist array.
  3. planner patch <plan.md> <patch.json> <output.md> --section verification --subsection {summary|automated|manual}

Trap:
  Full-step replacement re-escapes every diff in that step, even if only one FileChange needed a change.
  Prefer --field <leaf> for in-place edits. Whole-FileChange replacement (--subsection N --file F without --field) is rejected with --file requires --field.
`

func main() {
	os.Exit(Execute(os.Args[1:], os.Stdout, os.Stderr))
}

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
	case "validate":
		return runValidate(args[1:], stdout, stderr)
	case "create":
		return runCreate(args[1:], stdout, stderr)
	case "inspect":
		return runInspect(args[1:], stdout, stderr)
	case "patch":
		return runReplace(args[1:], stdout, stderr)
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
		default:
			return opts, fmt.Errorf("unknown flag %q", args[i])
		}
	}
	if opts.md && opts.jsonMode {
		return opts, fmt.Errorf("--md and --json are mutually exclusive")
	}
	if opts.md && (opts.section != "" || opts.subsection != "" || opts.file != "" || opts.field != "") {
		return opts, fmt.Errorf("--md does not accept selectors")
	}
	if opts.subsection != "" && opts.section == "" {
		return opts, fmt.Errorf("--subsection requires --section")
	}
	if opts.section != "" && !opts.jsonMode && opts.field != "diff" {
		return opts, fmt.Errorf("--section requires --json")
	}
	if !opts.md && !opts.jsonMode && opts.field != "diff" {
		return opts, fmt.Errorf("either --md or --json is required")
	}
	return opts, nil
}

// validateFieldGrammar is the shared leaf-selector validator for patch and
// template. It keeps both commands aligned on the same section/subsection/file
// and field combinations.
func validateFieldGrammar(opts replace.ReplaceOptions) error {
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
	if err := validateFieldGrammar(replace.ReplaceOptions{
		Section:    opts.section,
		Subsection: opts.subsection,
		File:       opts.file,
		Field:      opts.field,
	}); err != nil {
		reportError(stderr, "template", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}

	plan := schema.BuildPlanTemplate()
	switch {
	case opts.md:
		rendered, err := render.RenderPlan(plan)
		if err != nil {
			reportError(stderr, "template", newPlannerCLIError(PlannerRenderOutputError, err, "plan markdown"))
			return 1
		}
		_, _ = io.WriteString(stdout, rendered)
		return 0
	case opts.section == "":
		raw, err := schema.MarshalJSONNoEscape(plan)
		if err != nil {
			reportError(stderr, "template", newPlannerCLIError(PlannerRenderOutputError, err, "template JSON"))
			return 1
		}
		_, _ = stdout.Write(append(raw, '\n'))
		return 0
	case opts.field == "diff":
		_, _ = stdout.Write([]byte("--- a/<path>\n+++ b/<path>\n@@ -1 +1 @@\n-old\n+new\n"))
		return 0
	default:
		raw, err := schema.MarshalSection(plan, opts.section, opts.subsection, opts.file, opts.field)
		if err != nil {
			reportError(stderr, "template", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		_, _ = stdout.Write(raw)
		return 0
	}
}

func runValidate(args []string, stdout io.Writer, stderr io.Writer) int {
	positional, pf, err := splitPreviewArgs(args, false, true)
	if err != nil {
		reportError(stderr, "validate", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if (len(positional) == 0 && !pf.stdin && !stdinPiped()) || len(positional) > 1 {
		reportError(stderr, "validate", newPlannerCLIError(PlannerUsageError, nil, "usage: planner validate [<plan.json>] [--stdin]"))
		return 2
	}
	plan, err := readPlanFrom(positional, pf.stdin, stderr)
	if err != nil {
		reportError(stderr, "validate", err)
		return plannerExitCode(err)
	}
	if err := validate.ValidatePlan(plan); err != nil {
		reportError(stderr, "validate", newPlannerCLIError(PlannerValidateInputError, err, "plan"))
		return 1
	}
	_, _ = io.WriteString(stdout, "OK\n")
	return 0
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
	rendered, err := render.RenderPlan(plan)
	if err != nil {
		reportError(stderr, "create", newPlannerCLIError(PlannerRenderOutputError, err, "plan markdown"))
		return 1
	}
	if err := validate.ValidatePlan(plan); err != nil {
		reportError(stderr, "create", newPlannerCLIError(PlannerValidateInputError, err, "plan"))
		return 1
	}
	if err := validate.VerifyRenderedText(rendered, plan); err != nil {
		reportError(stderr, "create", newPlannerCLIError(PlannerValidateInputError, err, "rendered plan"))
		return 1
	}
	return runPreview(stdout, stderr, pf, rendered, outputPath, "create", func() error {
		if err := replace.WriteAtomic(outputPath, []byte(rendered)); err != nil {
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
	for _, rule := range schema.ValidationRules() {
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

	plan, _, _, _, err := inspect.ParseMarkdown(string(raw))
	if err != nil {
		reportError(stderr, "inspect", newPlannerCLIError(PlannerDecodeInputError, err, "plan markdown"))
		return 1
	}

	out, err := json.MarshalIndent(plan, "", "  ")
	if err != nil {
		reportError(stderr, "inspect", newPlannerCLIError(PlannerWriteOutputError, err, "inspect JSON"))
		return 1
	}
	_, _ = stdout.Write(append(out, '\n'))
	return 0
}

func runReplace(args []string, stdout io.Writer, stderr io.Writer) int {
	for _, a := range args {
		if a == "--help" || a == "-h" {
			_, _ = io.WriteString(stdout, patchHelpText)
			return 0
		}
	}
	positional, pf, err := splitPreviewArgs(args, true, true)
	if err != nil {
		reportError(stderr, "patch", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	var sourcePath, patchPath, outputPath string
	var flags []string
	// positional still contains --section / --subsection / --append after splitPreviewArgs
	// strips only --stdin/--diff/--dry-run. Count path args (2 with --stdin, 3 otherwise).
	pathCount := 3
	if pf.stdin {
		pathCount = 2
	}
	if len(positional) < pathCount {
		reportError(stderr, "patch", newPlannerCLIError(PlannerUsageError, nil, "usage: planner patch <plan.md> [<patch.json>|<diff.txt>] <output.md> --section <section> [--subsection <name-or-index>] [--file <filename>] [--field <field>] [--append] [--stdin] [--diff] [--dry-run]"))
		return 2
	}
	switch pathCount {
	case 3:
		sourcePath, patchPath, outputPath = positional[0], positional[1], positional[2]
		flags = positional[3:]
	case 2:
		sourcePath, outputPath = positional[0], positional[1]
		flags = positional[2:]
	}

	opts, err := parseReplaceOptions(flags)
	if err != nil {
		reportError(stderr, "patch", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}

	var patchData []byte
	if opts.Field != "" {
		patchData, err = readRawSource(patchPath, pf.stdin)
	} else {
		patchData, _, err = readJSONSource(patchPath, pf.stdin, false, stderr)
	}
	if err != nil {
		reportError(stderr, "patch", newPlannerCLIError(PlannerReadInputError, err, patchSourceLabel(patchPath, pf.stdin)))
		return 1
	}

	out, result, err := replace.PreviewFromData(sourcePath, opts, patchData)
	if err != nil {
		cliErr := mapReplaceCLIError(err, sourcePath)
		reportError(stderr, "patch", cliErr)
		return plannerExitCode(cliErr)
	}

	exit := runPreviewAgainstSource(stdout, stderr, pf, out, sourcePath, outputPath, "patch", func() error {
		if err := replace.WriteAtomic(outputPath, []byte(out)); err != nil {
			return newPlannerCLIError(PlannerWriteOutputError, err, outputPath)
		}
		return nil
	})
	if exit != 0 || pf.dryRun || pf.diff {
		return exit
	}

	enc := json.NewEncoder(stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(result); err != nil {
		reportError(stderr, "patch", newPlannerCLIError(PlannerWriteOutputError, err, "result JSON"))
		return 1
	}
	return 0
}

func parseReplaceOptions(flags []string) (replace.ReplaceOptions, error) {
	opts := replace.ReplaceOptions{}
	for i := 0; i < len(flags); i++ {
		switch flags[i] {
		case "--section", "-s":
			i++
			if i >= len(flags) {
				return opts, fmt.Errorf("missing value for --section")
			}
			opts.Section = flags[i]
		case "--subsection":
			i++
			if i >= len(flags) {
				return opts, fmt.Errorf("missing value for --subsection")
			}
			opts.Subsection = flags[i]
		case "--file":
			i++
			if i >= len(flags) {
				return opts, fmt.Errorf("missing value for --file")
			}
			opts.File = flags[i]
		case "--field":
			i++
			if i >= len(flags) {
				return opts, fmt.Errorf("missing value for --field")
			}
			opts.Field = flags[i]
		case "--append":
			opts.Append = true
		default:
			return opts, fmt.Errorf("unknown flag %q", flags[i])
		}
	}
	if opts.Section == "" {
		return opts, fmt.Errorf("--section is required")
	}
	if opts.Section == "implementation" && opts.Subsection != "" && !opts.Append {
		if _, err := strconv.Atoi(opts.Subsection); err != nil {
			return opts, fmt.Errorf("--subsection for implementation must be a 1-based integer index")
		}
	}
	if err := validateFieldGrammar(opts); err != nil {
		return opts, err
	}
	return opts, nil
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
	data, repaired, err := jsoninput.Read(path, useStdin, allowAutoDetect, os.Stdin, stdinPiped)
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

// readPlanFrom is the plan-decoding wrapper for runValidate/runCreate.
// Decode errors are wrapped in typed planner CLI errors so tests can assert on
// stable failure categories instead of raw strings.
func readPlanFrom(positional []string, useStdin bool, stderr io.Writer) (schema.Plan, error) {
	path := ""
	if len(positional) > 0 {
		path = positional[0]
	}
	data, _, err := readJSONSource(path, useStdin, true, stderr)
	if err != nil {
		return schema.Plan{}, newPlannerCLIError(PlannerReadInputError, err, patchSourceLabel(path, useStdin))
	}
	plan, err := schema.DecodePlan(data)
	if err != nil {
		return schema.Plan{}, newPlannerCLIError(PlannerDecodeInputError, err, "plan JSON")
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
	var replaceErr *replace.ReplaceError
	if !errors.As(err, &replaceErr) {
		return newPlannerCLIError(PlannerValidateInputError, err, "result")
	}
	switch replaceErr.Code {
	case replace.ReplaceInvalidOptionsError:
		return newPlannerCLIError(PlannerUsageError, err, err.Error())
	case replace.ReplaceReadSourceError:
		return newPlannerCLIError(PlannerReadInputError, err, sourcePath)
	case replace.ReplaceParseSourceError:
		return newPlannerCLIError(PlannerDecodeInputError, err, "plan markdown")
	case replace.ReplaceDecodePatchError:
		return newPlannerCLIError(PlannerDecodeInputError, err, "patch JSON")
	case replace.ReplaceRenderResultError:
		return newPlannerCLIError(PlannerRenderOutputError, err, "updated plan markdown")
	case replace.ReplaceValidateResultError:
		return newPlannerCLIError(PlannerValidateInputError, err, "updated plan")
	case replace.ReplaceFileNotFoundError:
		e := newPlannerCLIError(PlannerUsageError, err, err.Error())
		e.RecoveryHint = "run planner inspect <plan.md> to list valid filenames in the targeted step"
		return e
	case replace.ReplaceFileAmbiguousError:
		e := newPlannerCLIError(PlannerUsageError, err, err.Error())
		e.RecoveryHint = "rename or consolidate duplicate FileChange filenames before patching"
		return e
	case replace.ReplaceParseSplicedSourceError:
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
