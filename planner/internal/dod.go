package internal

import (
	"fmt"
	"strings"
)

func runDoDEdit(ctx editContext) int {
	ctx.cmd = "dod"
	pos := ctx.flags.positional
	if len(pos) < 1 {
		reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, "usage: planner dod <narrative|current-state|module-shape|goal> ..."))
		return 2
	}
	sub := pos[0]
	subMap := map[string]string{"narrative": "narrative", "current-state": "current_state", "module-shape": "module_shape"}
	if ss, ok := subMap[sub]; ok {
		if err := ctx.flags.rejectValueFlagsExcept(); err != nil {
			reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		var text []string
		var err error
		ctx, text, err = requirePositional(ctx, []string{sub, "set"}, 2, 3)
		if err != nil {
			reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		v, err := scalarValue(ctx, text, true)
		if err != nil {
			reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		return runEditPreview(ctx, ReplaceOptions{Section: "definition_of_done", Subsection: ss, Raw: true}, v)
	}
	if sub != "goal" || len(pos) < 2 {
		reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("unknown dod target: %s", sub)))
		return 2
	}
	if err := rejectStdinForStructured(ctx); err != nil {
		reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if err := rejectDiffStdin(ctx); err != nil {
		reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	action := pos[1]
	maxTail := 2
	if action == "add" || action == "set" {
		maxTail = 3
	}
	var tail []string
	var err error
	ctx, tail, err = requirePositional(ctx, []string{"goal", action}, 2, maxTail)
	if err != nil {
		reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	allowed := []string{}
	if action == "set" || action == "remove" {
		allowed = []string{"--goal"}
	}
	if err := ctx.flags.rejectValueFlagsExcept(allowed...); err != nil {
		reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	plan, err := readPlanForEdit(ctx.sourcePath)
	if err != nil {
		return reportEditError(ctx, "dod", err)
	}
	goals := append([]ChecklistItem(nil), plan.DefinitionOfDone.Goals...)
	switch action {
	case "add":
		if len(tail) != 1 || len(tail) > 0 && trimEmpty(tail[0]) {
			reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, "missing required text"))
			return 2
		}
		goals = append(goals, ChecklistItem{Text: tail[0]})
	case "set":
		idx, err := ctx.flags.index("--goal")
		if err != nil {
			reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		if idx > len(goals) {
			reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--goal %d out of range", idx)))
			return 2
		}
		if len(tail) != 1 || trimEmpty(tail[0]) {
			reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, "missing required text"))
			return 2
		}
		goals[idx-1].Text = tail[0]
	case "remove":
		if len(goals) == 1 {
			reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, "cannot remove the final definition_of_done goal"))
			return 2
		}
		idx, err := ctx.flags.index("--goal")
		if err != nil {
			reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		if idx > len(goals) {
			reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--goal %d out of range", idx)))
			return 2
		}
		goals = append(goals[:idx-1], goals[idx:]...)
	default:
		reportError(ctx.stderr, "dod", newPlannerCLIError(PlannerUsageError, nil, "usage: planner dod goal add|set|remove"))
		return 2
	}
	return runEditPreview(ctx, ReplaceOptions{Section: "definition_of_done", Subsection: "goals"}, jsonBytes(goals))
}

func trimEmpty(s string) bool { return strings.TrimSpace(s) == "" }
