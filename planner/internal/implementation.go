package internal

import "fmt"

func runImplementationEdit(ctx editContext, args []string) int {
	ctx.cmd = "implementation"
	if len(args) < 2 || args[0] != "step" { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "usage: planner implementation step ...")); return 2 }
	if args[1] == "file-change" { return runFileChangeEdit(ctx, args[2:]) }
	action := args[1]
	if action == "title" || action == "summary" {
		if len(args) < 3 || args[2] != "set" { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "usage: planner implementation step "+action+" set")); return 2 }
		if err := rejectDiffStdin(ctx); err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		step, err := ctx.flags.index("--step"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		v, err := scalarValue(ctx, "--value", "--"+action); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		return runEditPreview(ctx, ReplaceOptions{Section:"implementation", Subsection:fmt.Sprint(step), Field:action, Raw:true}, v)
	}
	if err := rejectStdinForStructured(ctx); err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	plan, err := readPlanForEdit(ctx.sourcePath); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerReadInputError, err, ctx.sourcePath)); return 1 }
	steps := append([]Step(nil), plan.Implementation...)
	switch action {
	case "add":
		diff, ok := readStructuredDiff(ctx); if !ok { return 2 }
		st, ok := buildStep(ctx, diff); if !ok { return 2 }
		return runEditPreview(ctx, ReplaceOptions{Section:"implementation", Append:true}, mustJSON(st))
	case "remove":
		if err := rejectDiffStdin(ctx); err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		if len(steps) == 1 { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "cannot remove final implementation step; at least one step is required")); return 2 }
		idx, err := ctx.flags.index("--step"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		if idx > len(steps) { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--step %d out of range", idx))); return 2 }
		steps = append(steps[:idx-1], steps[idx:]...)
		return runEditPreview(ctx, ReplaceOptions{Section:"implementation"}, mustJSON(steps))
	default:
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "usage: planner implementation step add|remove|title set|summary set|file-change ...")); return 2
	}
}

func buildStep(ctx editContext, diff string) (Step, bool) { title, err := ctx.flags.stringFlag("--title"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return Step{}, false }; summary, err := ctx.flags.stringFlag("--summary"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return Step{}, false }; fn, err := ctx.flags.stringFlag("--filename", "--file"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return Step{}, false }; exp, err := ctx.flags.stringFlag("--explanation"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return Step{}, false }; return Step{Title:title, Summary:summary, FileChanges:[]FileChange{{Filename:fn, Explanation:exp, Diff:diff}}}, true }

func readStructuredDiff(ctx editContext) (string, bool) { if ctx.flags.diffStdin { b, err := readRawSource("", true); if err != nil { reportError(ctx.stderr, ctx.cmd, newPlannerCLIError(PlannerReadInputError, err, "stdin")); return "", false }; return string(b), true }; v, err := ctx.flags.stringFlag("--diff"); if err != nil { reportError(ctx.stderr, ctx.cmd, newPlannerCLIError(PlannerUsageError, err, err.Error())); return "", false }; return v, true }

func runFileChangeEdit(ctx editContext, args []string) int {
	if len(args) < 1 { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "usage: planner implementation step file-change ...")); return 2 }
	action := args[0]
	if action == "filename" || action == "explanation" || action == "diff" {
		if len(args) < 2 || args[1] != "set" { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "usage: planner implementation step file-change "+action+" set")); return 2 }
		if action != "diff" { if err := rejectDiffStdin(ctx); err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 } }
		step, err := ctx.flags.index("--step"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		change, err := ctx.flags.index("--change"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		if action == "diff" { if ctx.flags.diffStdin { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "--diff-stdin is only valid for structured add commands")); return 2 }; b, err := readRawSource("", ctx.flags.stdin); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerReadInputError, err, "stdin")); return 1 }; return runEditPreview(ctx, ReplaceOptions{Section:"implementation", Subsection:fmt.Sprint(step), Change:change, Field:"diff"}, b) }
		v, err := scalarValue(ctx, "--value", "--"+action); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		return runEditPreview(ctx, ReplaceOptions{Section:"implementation", Subsection:fmt.Sprint(step), Change:change, Field:action, Raw:true}, v)
	}
	if err := rejectStdinForStructured(ctx); err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	plan, err := readPlanForEdit(ctx.sourcePath); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerReadInputError, err, ctx.sourcePath)); return 1 }
	step, err := ctx.flags.index("--step"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	if step > len(plan.Implementation) { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--step %d out of range", step))); return 2 }
	updated := plan.Implementation[step-1]
	switch action {
	case "add":
		diff, ok := readStructuredDiff(ctx); if !ok { return 2 }
		fn, err := ctx.flags.stringFlag("--filename", "--file"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		exp, err := ctx.flags.stringFlag("--explanation"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		updated.FileChanges = append(updated.FileChanges, FileChange{Filename:fn, Explanation:exp, Diff:diff})
	case "remove":
		if err := rejectDiffStdin(ctx); err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		if len(updated.FileChanges) == 1 { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "cannot remove final file-change; at least one file-change is required")); return 2 }
		change, err := ctx.flags.index("--change"); if err != nil { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		if change > len(updated.FileChanges) { reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--change %d out of range", change))); return 2 }
		updated.FileChanges = append(updated.FileChanges[:change-1], updated.FileChanges[change:]...)
	default:
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "usage: planner implementation step file-change add|remove|filename set|explanation set|diff set")); return 2
	}
	return runEditPreview(ctx, ReplaceOptions{Section:"implementation", Subsection:fmt.Sprint(step)}, mustJSON(updated))
}
