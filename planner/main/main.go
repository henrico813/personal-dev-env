package main

import (
	"encoding/json"
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
  planner validate <plan.json>
  planner create <input.json> <output.md>
  planner inspect <plan.md>
  planner replace <plan.md> <patch.json> <output.md> --section <section> [--subsection <name-or-index>] [--append]

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
  --section/-s <section>        Required. One of: overview, definition_of_done, implementation, verification
  --subsection/-i <name-or-index>  Optional. Field name for definition_of_done; 1-based step index for implementation
  --append                      Optional. Append a new step to implementation

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
	if len(args) != 1 {
		fmt.Fprintln(stderr, "usage: planner validate <plan.json>")
		return 2
	}
	plan, err := validate.ReadPlanFile(args[0])
	if err != nil {
		fmt.Fprintf(stderr, "validate %s: %v\n", args[0], err)
		return 1
	}
	if err := validate.ValidatePlan(plan); err != nil {
		fmt.Fprintf(stderr, "validate %s: %v\n", args[0], err)
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
	if len(args) != 2 {
		fmt.Fprintln(stderr, "usage: planner create <plan.json> <output.md>")
		return 2
	}
	if err := render.CreatePlan(args[0], args[1]); err != nil {
		fmt.Fprintf(stderr, "create: %v\n", err)
		return 1
	}
	_, _ = io.WriteString(stdout, args[1]+"\n")
	return 0
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
	if len(args) < 3 {
		fmt.Fprintln(stderr, "usage: planner replace <plan.md> <patch.json> <output.md> --section <section> [--subsection <name-or-index>] [--append]")
		return 2
	}
	sourcePath := args[0]
	patchPath := args[1]
	outputPath := args[2]
	flags := args[3:]

	opts, err := parseReplaceOptions(flags)
	if err != nil {
		fmt.Fprintf(stderr, "replace: %v\n", err)
		return 2
	}

	contract, err := replace.Run(sourcePath, opts, patchPath, outputPath)
	if err != nil {
		fmt.Fprintf(stderr, "replace: %v\n", err)
		return 1
	}

	enc := json.NewEncoder(stdout)
	enc.SetIndent("", "  ")
	if err := enc.Encode(contract); err != nil {
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
		case "--subsection", "-i":
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
