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
	"planner/render"
	"planner/replace"
	"planner/schema"
	"planner/validate"
)

const helpText = `planner generates implementation-plan markdown from canonical JSON.

Usage:
  planner
  planner help
  planner show-schema
  planner validate [<plan.json>] [--stdin]
  planner create [<plan.json>] <output.md> [--stdin] [--diff] [--write]
  planner inspect <plan.md>
  planner replace <plan.md> [<patch.json>] <output.md> --section <section> [--subsection <name-or-index>] [--append] [--stdin] [--diff] [--write]

Scratch flow:
  1. Research the task.
  2. Run planner show-schema.
  3. Write plan JSON that matches planner show-schema.
  4. Run planner validate <plan.json>.
  5. Run planner create <plan.json> <output.md>.

Rewrite flow (full rewrite):
  1. Read the existing markdown issue.
  2. Map its content into canonical JSON matching planner show-schema.
  3. Run planner validate <plan.json>.
  4. Run planner create <plan.json> <output.md>.
  5. Compare the rendered issue with the source issue for dropped content.

Partial update flow:
  1. Run planner inspect <plan.md> to get section and step spans.
  2. Write patch JSON for the target scope.
  3. Run planner replace <plan.md> <patch.json> <output.md> --section <section>.
  4. Non-targeted sections remain byte-for-byte unchanged.

replace flags:
  --section/-s <section>           Required. One of: overview, definition_of_done, implementation, verification
  --subsection <name-or-index>     Optional. Field name for definition_of_done; 1-based step index for implementation
  --append                         Optional. Append a new step to implementation
  --stdin                          Optional. Read patch JSON from stdin instead of a file
  --diff                           Optional. Print diff of would-be change; exit 1 if non-empty, 0 if unchanged
  --write                          Optional. Write the output (default when neither --diff nor --write is set)

show-schema contract:
  - Prints a JSON object with plan_example and validation_rules.
  - Use only plan_example as input to planner validate and planner create.
  - validation_rules lists the semantic rules the current validator enforces.
  - Includes command semantics for help, show-schema, validate, create, inspect, and replace.

Validation rules:
`

func main() {
	os.Exit(Execute(os.Args[1:], os.Stdout, os.Stderr))
}

// Execute is the production command entrypoint used by main() and CLI tests.
func Execute(args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) == 0 {
		printHelp(stdout)
		return 0
	}

	switch args[0] {
	case "help", "--help", "-h":
		printHelp(stdout)
		return 0
	case "show-schema":
		return runShowSchema(stdout, stderr)
	case "validate":
		return runValidate(args[1:], stdout, stderr)
	case "create":
		return runCreate(args[1:], stdout, stderr)
	case "inspect":
		return runInspect(args[1:], stdout, stderr)
	case "replace":
		return runReplace(args[1:], stdout, stderr)
	default:
		fmt.Fprintf(stderr, "unknown command: %s\n\n", args[0])
		printHelp(stderr)
		return 2
	}
}

func runShowSchema(stdout io.Writer, stderr io.Writer) int {
	schemaJSON := schema.BuildSchemaJSON()
	if _, err := io.WriteString(stdout, schemaJSON+"\n"); err != nil {
		fmt.Fprintf(stderr, "write schema: %v\n", err)
		return 1
	}
	return 0
}

func runValidate(args []string, stdout io.Writer, stderr io.Writer) int {
	positional, pf, err := splitPreviewArgs(args, false, true)
	if err != nil || (len(positional) == 0 && !pf.stdin && !stdinPiped()) || len(positional) > 1 {
		fmt.Fprintln(stderr, "usage: planner validate [<plan.json>] [--stdin]")
		return 2
	}
	plan, err := readPlanFrom(positional, pf.stdin, stderr)
	if err != nil {
		fmt.Fprintf(stderr, "validate: %v\n", err)
		return 1
	}
	if err := validate.ValidatePlan(plan); err != nil {
		fmt.Fprintf(stderr, "validate: %v\n", err)
		return 1
	}
	_, _ = io.WriteString(stdout, "OK\n")
	return 0
}

func runCreate(args []string, stdout io.Writer, stderr io.Writer) int {
	if len(args) >= 1 && args[0] == "step" {
		fmt.Fprintln(stderr, "planner create step is no longer supported; rewrite the full plan JSON and run planner create <plan.json> <output.md>")
		return 2
	}
	positional, pf, err := splitPreviewArgs(args, true, true)
	if err != nil {
		fmt.Fprintf(stderr, "create: %v\n", err)
		return 2
	}
	var inputPath, outputPath string
	switch len(positional) {
	case 2:
		inputPath, outputPath = positional[0], positional[1]
	case 1:
		outputPath = positional[0]
	default:
		fmt.Fprintln(stderr, "usage: planner create [<plan.json>] <output.md> [--stdin] [--diff] [--write]")
		return 2
	}
	plan, err := readPlanFrom(filterNonEmpty([]string{inputPath}), pf.stdin, stderr)
	if err != nil {
		fmt.Fprintf(stderr, "create: %v\n", err)
		return 1
	}
	rendered, err := render.RenderPlan(plan)
	if err != nil {
		fmt.Fprintf(stderr, "create: %v\n", err)
		return 1
	}
	if err := validate.ValidatePlan(plan); err != nil {
		fmt.Fprintf(stderr, "create: %v\n", err)
		return 1
	}
	if err := validate.VerifyRenderedText(rendered, plan); err != nil {
		fmt.Fprintf(stderr, "create: %v\n", err)
		return 1
	}
	return runPreview(stdout, stderr, pf, rendered, outputPath, "create", func() error {
		return replace.WriteAtomic(outputPath, []byte(rendered))
	}, outputPath)
}

func printHelp(w io.Writer) {
	io.WriteString(w, buildHelpText())
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
		fmt.Fprintln(stderr, "usage: planner inspect <plan.md>")
		return 2
	}
	raw, err := os.ReadFile(args[0])
	if err != nil {
		fmt.Fprintf(stderr, "inspect: %v\n", err)
		return 1
	}

	plan, sectionSpans, stepSpans, err := inspect.ParseMarkdown(string(raw))
	if err != nil {
		fmt.Fprintf(stderr, "inspect: %v\n", err)
		return 1
	}

	resp := struct {
		Title              string         `json:"title"`
		Sections           []string       `json:"sections"`
		ImplementationSize int            `json:"implementation_size"`
		OverviewSpan       inspect.Span   `json:"overview_span"`
		DoDSpan            inspect.Span   `json:"definition_of_done_span"`
		ImplSectionSpan    inspect.Span   `json:"implementation_section_span"`
		ImplStepSpans      []inspect.Span `json:"implementation_step_spans"`
		VerificationSpan   inspect.Span   `json:"verification_span"`
	}{
		Title:              plan.Title,
		Sections:           []string{"overview", "definition_of_done", "implementation", "verification"},
		ImplementationSize: len(plan.Implementation),
		OverviewSpan:       sectionSpans.Overview,
		DoDSpan:            sectionSpans.DefinitionOfDone,
		ImplSectionSpan:    sectionSpans.Implementation,
		ImplStepSpans:      stepSpans,
		VerificationSpan:   sectionSpans.Verification,
	}

	enc := json.NewEncoder(stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(resp); err != nil {
		fmt.Fprintf(stderr, "inspect: %v\n", err)
		return 1
	}
	return 0
}

func runReplace(args []string, stdout io.Writer, stderr io.Writer) int {
	positional, pf, err := splitPreviewArgs(args, true, true)
	if err != nil {
		fmt.Fprintf(stderr, "replace: %v\n", err)
		return 2
	}
	var sourcePath, patchPath, outputPath string
	var flags []string
	// positional still contains --section / --subsection / --append after splitPreviewArgs
	// strips only --stdin/--diff/--write. Count path args (2 with --stdin, 3 otherwise).
	pathCount := 3
	if pf.stdin {
		pathCount = 2
	}
	if len(positional) < pathCount {
		fmt.Fprintln(stderr, "usage: planner replace <plan.md> [<patch.json>] <output.md> --section <section> [--subsection <name-or-index>] [--append] [--stdin] [--diff] [--write]")
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
		fmt.Fprintf(stderr, "replace: %v\n", err)
		return 2
	}

	patchData, err := readJSONSource(patchPath, pf.stdin, false, stderr)
	if err != nil {
		fmt.Fprintf(stderr, "replace: %v\n", err)
		return 1
	}

	out, result, err := replace.PreviewFromData(sourcePath, opts, patchData)
	if err != nil {
		fmt.Fprintf(stderr, "replace: %v\n", err)
		return 1
	}

	exit := runPreviewAgainstSource(stdout, stderr, pf, out, sourcePath, outputPath, "replace", func() error {
		return replace.WriteAtomic(outputPath, []byte(out))
	})
	if exit != 0 || !pf.write || pf.diff {
		return exit
	}

	enc := json.NewEncoder(stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(result); err != nil {
		fmt.Fprintf(stderr, "replace: %v\n", err)
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
	return opts, nil
}

// previewFlags carries --stdin, --diff, --write. splitPreviewArgs extracts
// them from the flag tail before the subcommand-specific parser runs, so the
// existing strict parsers never see an unknown flag.
type previewFlags struct {
	stdin bool
	diff  bool
	write bool
}

// splitPreviewArgs separates --stdin/--diff/--write from positional and
// subcommand flags. When allowPreview is false, --diff/--write are passed
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
		case a == "--write" && allowPreview:
			pf.write = true
		default:
			kept = append(kept, a)
		}
	}
	if allowPreview && !pf.diff && !pf.write {
		pf.write = true
	}
	return kept, pf, nil
}

// readJSONSource returns JSON bytes for a subcommand's input. When --stdin is
// set, reads stdin. When allowAutoDetect is true (validate/create only) and no
// path is supplied and stdin is piped, reads stdin. Otherwise reads the path.
func readJSONSource(path string, useStdin, allowAutoDetect bool, stderr io.Writer) ([]byte, error) {
	if useStdin || (allowAutoDetect && path == "" && stdinPiped()) {
		if !stdinPiped() {
			fmt.Fprintln(stderr, "planner: reading JSON from stdin (Ctrl-D to end)")
		}
		return io.ReadAll(os.Stdin)
	}
	if path == "" {
		return nil, fmt.Errorf("no JSON source: pass a path, pipe stdin, or use --stdin")
	}
	return os.ReadFile(path)
}

// readPlanFrom is the plan-decoding wrapper for runValidate/runCreate.
func readPlanFrom(positional []string, useStdin bool, stderr io.Writer) (schema.Plan, error) {
	path := ""
	if len(positional) > 0 {
		path = positional[0]
	}
	data, err := readJSONSource(path, useStdin, true, stderr)
	if err != nil {
		return schema.Plan{}, err
	}
	return schema.DecodePlan(data)
}

// stdinPiped reports whether os.Stdin has piped data (not a terminal).
func stdinPiped() bool {
	fi, err := os.Stdin.Stat()
	return err == nil && (fi.Mode()&os.ModeCharDevice) == 0
}

// runPreview orchestrates --diff / --write for create. baseline is outputPath.
// Exit 0 means success or no diff; 1 means diff produced (only when --diff and
// not --write); 2 means error (propagated from doWrite). stdoutPathOnWrite is
// printed on successful --write when --diff is not set, preserving the legacy
// "create prints the output path on success" stdout contract.
func runPreview(stdout, stderr io.Writer, pf previewFlags, rendered, basePath, cmdName string, doWrite func() error, stdoutPathOnWrite string) int {
	baseline, err := readBaseline(basePath)
	if err != nil {
		fmt.Fprintf(stderr, "%s: %v\n", cmdName, err)
		return 2
	}
	d := diffLines(baseline, rendered)
	if pf.diff && d != "" {
		_, _ = io.WriteString(stdout, d)
	}
	if pf.write {
		if err := doWrite(); err != nil {
			fmt.Fprintf(stderr, "%s: %v\n", cmdName, err)
			return 2
		}
		if !pf.diff && stdoutPathOnWrite != "" {
			_, _ = io.WriteString(stdout, stdoutPathOnWrite+"\n")
		}
		return 0
	}
	if d != "" {
		return 1
	}
	return 0
}

// runPreviewAgainstSource is runPreview with sourcePath as the baseline,
// used by replace so the diff shows what the patch changes in the source,
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
