package internal

import "fmt"

func runVerificationEdit(ctx editContext, args []string) int {
	ctx.cmd = "verification"
	if len(args) < 2 { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, "usage: planner verification summary set|automated add|set|remove|manual add|set|remove")); return 2 }
	target, action := args[0], args[1]
	if target == "summary" {
		if action != "set" { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, "usage: planner verification summary set <plan.md> <output.md> --value <text>")); return 2 }
		if err := rejectDiffStdin(ctx); err != nil { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		var v []byte
		if ctx.flags.stdin { b, err := readRawScalar("", true); if err != nil { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerReadInputError, err, "stdin")); return 1 }; v = b } else { s := ctx.flags.optional("--value", "--summary"); v = []byte(s) }
		return runEditPreview(ctx, ReplaceOptions{Section:"verification", Subsection:"summary", Raw:true}, v)
	}
	if target != "automated" && target != "manual" { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("unknown verification target: %s", target))); return 2 }
	if err := rejectStdinForStructured(ctx); err != nil { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	if err := rejectDiffStdin(ctx); err != nil { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
	plan, err := readPlanForEdit(ctx.sourcePath); if err != nil { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerReadInputError, err, ctx.sourcePath)); return 1 }
	if plan.Verification == nil { plan.Verification = &Verification{} }
	items := append([]ChecklistItem(nil), plan.Verification.Automated...)
	if target == "manual" { items = append([]ChecklistItem(nil), plan.Verification.Manual...) }
	switch action {
	case "add":
		text, err := ctx.flags.stringFlag("--text", "--value"); if err != nil { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		items = append(items, ChecklistItem{Text:text})
	case "set":
		idx, err := ctx.flags.index("--index"); if err != nil { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		if idx > len(items) { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--index %d out of range", idx))); return 2 }
		text, err := ctx.flags.stringFlag("--text", "--value"); if err != nil { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		items[idx-1].Text = text
	case "remove":
		idx, err := ctx.flags.index("--index"); if err != nil { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, err, err.Error())); return 2 }
		if idx > len(items) { reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--index %d out of range", idx))); return 2 }
		items = append(items[:idx-1], items[idx:]...)
	default:
		reportError(ctx.stderr, "verification", newPlannerCLIError(PlannerUsageError, nil, "usage: planner verification automated|manual add|set|remove")); return 2
	}
	return runEditPreview(ctx, ReplaceOptions{Section:"verification", Subsection:target}, mustJSON(items))
}
