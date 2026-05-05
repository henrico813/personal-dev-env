package internal

func runTitleEdit(ctx editContext) int {
	ctx.cmd = "title"
	if err := ctx.flags.rejectValueFlagsExcept(); err != nil {
		reportError(ctx.stderr, "title", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	var text []string
	var err error
	ctx, text, err = requirePositional(ctx, []string{"set"}, 2, 3)
	if err != nil {
		reportError(ctx.stderr, "title", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	v, err := scalarValue(ctx, text, true)
	if err != nil {
		reportError(ctx.stderr, "title", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	return runEditPreview(ctx, ReplaceOptions{Section: "title", Raw: true}, v)
}
