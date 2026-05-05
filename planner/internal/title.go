package internal

func runTitleEdit(ctx editContext, args []string) int {
	ctx.cmd = "title"
	if len(args) < 1 || args[0] != "set" { reportError(ctx.stderr, "title", newPlannerCLIError(PlannerUsageError, nil, "usage: planner title set <plan.md> <output.md> --value <text> [--stdin]")); return 2 }
	if err := rejectDiffStdin(ctx); err != nil { reportError(ctx.stderr, "title", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	v, err := scalarValue(ctx, "--value", "--title")
	if err != nil { reportError(ctx.stderr, "title", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	return runEditPreview(ctx, ReplaceOptions{Section:"title", Raw:true}, v)
}
