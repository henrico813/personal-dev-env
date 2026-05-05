package internal

import "fmt"

func runDoDEdit(ctx editContext, args []string) int {
	ctx.cmd = "dod"
	if len(args) < 2 { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, "usage: planner dod <narrative|current-state|module-shape|goal> ...")); return 2 }
	sub, action := args[0], args[1]
	subMap := map[string]string{"narrative":"narrative", "current-state":"current_state", "module-shape":"module_shape"}
	if ss, ok := subMap[sub]; ok {
		if action != "set" { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, "usage: planner dod "+sub+" set <plan.md> <output.md> --value <text>")); return 2 }
		if err := rejectDiffStdin(ctx); err != nil { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		v, err := scalarValue(ctx, "--value", "--text")
		if err != nil { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		return runEditPreview(ctx, ReplaceOptions{Section:"definition_of_done", Subsection:ss, Raw:true}, v)
	}
	if sub != "goal" { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("unknown dod target: %s", sub))); return 2 }
	if err := rejectStdinForStructured(ctx); err != nil { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	if err := rejectDiffStdin(ctx); err != nil { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	plan, err := readPlanForEdit(ctx.sourcePath); if err != nil { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerReadInputError, err, ctx.sourcePath)); return 1 }
	goals := append([]ChecklistItem(nil), plan.DefinitionOfDone.Goals...)
	switch action {
	case "add":
		text, err := ctx.flags.stringFlag("--text", "--goal", "--value"); if err != nil { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		goals = append(goals, ChecklistItem{Text:text})
	case "set":
		idx, err := ctx.flags.index("--index", "--goal"); if err != nil { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		if idx > len(goals) { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--index %d out of range", idx))); return 2 }
		text, err := ctx.flags.stringFlag("--text", "--value"); if err != nil { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		goals[idx-1].Text = text
	case "remove":
		if len(goals) == 1 { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, "cannot remove final DoD goal; at least one goal is required")); return 2 }
		idx, err := ctx.flags.index("--index", "--goal"); if err != nil { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		if idx > len(goals) { reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--index %d out of range", idx))); return 2 }
		goals = append(goals[:idx-1], goals[idx:]...)
	default:
		reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, "usage: planner dod goal add|set|remove")); return 2
	}
	return runEditPreview(ctx, ReplaceOptions{Section:"definition_of_done", Subsection:"goals"}, mustJSON(goals))
}
