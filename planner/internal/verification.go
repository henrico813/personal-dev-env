package internal

import "fmt"

func runVerificationEdit(ctx editContext) int {
	ctx.cmd = "verification"
	pos := ctx.flags.positional
	if len(pos) < 2 {
		reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, "usage: planner verification summary set|automated add|set|remove|manual add|set|remove"))
		return 2
	}
	target, action := pos[0], pos[1]
	if target == "summary" {
		if err := ctx.flags.rejectValueFlagsExcept(); err != nil {
			reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		var text []string
		var err error
		ctx, text, err = requirePositional(ctx, []string{"summary", "set"}, 2, 3)
		if err != nil {
			reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		v, err := scalarValue(ctx, text, false)
		if err != nil {
			reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		return runEditPreview(ctx, ReplaceOptions{Section: "verification", Subsection: "summary", Raw: true}, v)
	}
	if target != "automated" && target != "manual" {
		reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("unknown verification target: %s", target)))
		return 2
	}
	if err := rejectStdinForStructured(ctx); err != nil {
		reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if err := rejectDiffStdin(ctx); err != nil {
		reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	maxTail := 2
	if action == "add" || action == "set" {
		maxTail = 3
	}
	var tail []string
	var err error
	ctx, tail, err = requirePositional(ctx, []string{target, action}, 2, maxTail)
	if err != nil {
		reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	allowed := []string{}
	if action == "set" || action == "remove" {
		allowed = []string{"--item"}
	}
	if err := ctx.flags.rejectValueFlagsExcept(allowed...); err != nil {
		reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	plan, err := readPlanForEdit(ctx.sourcePath)
	if err != nil {
		return reportEditError(ctx, "verification", err)
	}
	if plan.Verification == nil {
		plan.Verification = &Verification{}
	}
	items := append([]ChecklistItem(nil), plan.Verification.Automated...)
	if target == "manual" {
		items = append([]ChecklistItem(nil), plan.Verification.Manual...)
	}
	switch action {
	case "add":
		if len(tail) != 1 || trimEmpty(tail[0]) {
			reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, "missing required text"))
			return 2
		}
		items = append(items, ChecklistItem{Text: tail[0]})
	case "set":
		idx, err := ctx.flags.index("--item")
		if err != nil {
			reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		if idx > len(items) {
			reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--item %d out of range", idx)))
			return 2
		}
		if len(tail) != 1 || trimEmpty(tail[0]) {
			reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, "missing required text"))
			return 2
		}
		items[idx-1].Text = tail[0]
	case "remove":
		idx, err := ctx.flags.index("--item")
		if err != nil {
			reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		if idx > len(items) {
			reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--item %d out of range", idx)))
			return 2
		}
		items = append(items[:idx-1], items[idx:]...)
	default:
		reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, "usage: planner verification automated|manual add|set|remove"))
		return 2
	}
	return runEditPreview(ctx, ReplaceOptions{Section: "verification", Subsection: target}, jsonBytes(items))
}
