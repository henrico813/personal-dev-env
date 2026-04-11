package main

import (
	"fmt"
	"io"
	"os"
)

const helpText = `planner generates implementation-plan markdown from canonical JSON.

Usage:
  planner
  planner help
  planner show-schema
  planner validate <plan.json>
  planner create <input.json> <output.md>

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

Current limitations:
  - planner renders markdown only and does not embed JSON appendices in rendered plans.
  - planner does not yet parse rendered markdown back into a Plan or provide planner check <plan.md>.

show-schema contract:
  - Includes the nested JSON shape the current validator recognizes.
  - Includes the required fields and constraints the current validator enforces.
  - Includes command semantics for help, show-schema, validate, and create.
`

func main() {
	os.Exit(run(os.Args[1:], os.Stdout, os.Stderr))
}

func run(args []string, stdout io.Writer, stderr io.Writer) int {
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
	default:
		fmt.Fprintf(stderr, "unknown command: %s\n\n", args[0])
		printHelp(stderr)
		return 2
	}
}

func runShowSchema(stdout io.Writer, stderr io.Writer) int {
	schemaJSON, err := buildSchemaJSON()
	if err != nil {
		fmt.Fprintf(stderr, "build schema: %v\n", err)
		return 1
	}
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
	plan, err := readPlanFile(args[0])
	if err != nil {
		fmt.Fprintf(stderr, "validate %s: %v\n", args[0], err)
		return 1
	}
	if err := validatePlan(plan); err != nil {
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
	if err := createPlan(args[0], args[1]); err != nil {
		fmt.Fprintf(stderr, "create: %v\n", err)
		return 1
	}
	_, _ = io.WriteString(stdout, args[1]+"\n")
	return 0
}

func printHelp(w io.Writer) {
	io.WriteString(w, helpText)
}
