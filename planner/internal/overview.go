package internal

func runOverviewEdit(ctx editContext, args []string) int {
	ctx.cmd = "overview"
	if len(args) < 1 || args[0] != "set" { reportError(ctx.stderr, "overview", newPlannerCLIError(PlannerUsageError, nil, "usage: planner overview set <plan.md> <output.md> --value <text> [--stdin]")); return 2 }
	if err := rejectDiffStdin(ctx); err != nil { reportError(ctx.stderr, "overview", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	v, err := scalarValue(ctx, "--value", "--overview")
	if err != nil { reportError(ctx.stderr, "overview", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	return runEditPreview(ctx, ReplaceOptions{Section:"overview", Raw:true}, v)
}
