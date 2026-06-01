package internal

import "fmt"

func runImplementationEdit(ctx editContext) int {
	ctx.cmd = "implementation"
	pos := ctx.flags.positional
	if len(pos) < 2 || pos[0] != "step" {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "usage: planner implementation step ..."))
		return 2
	}
	if pos[1] == "file-change" {
		return runFileChangeEdit(ctx)
	}
	if pos[1] == "title" || pos[1] == "summary" {
		action := pos[1]
		if err := ctx.flags.rejectValueFlagsExcept("--step"); err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		var text []string
		var err error
		ctx, text, err = requirePositional(ctx, []string{"step", action, "set"}, 2, 3)
		if err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		step, err := ctx.flags.index("--step")
		if err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		v, err := scalarValue(ctx, text, true)
		if err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		return runEditPreview(ctx, ReplaceOptions{Section: "implementation", Subsection: fmt.Sprint(step), Field: action, Raw: true}, v)
	}
	if err := rejectStdinForStructured(ctx); err != nil {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	action := pos[1]
	var err error
	ctx, _, err = requirePositional(ctx, []string{"step", action}, 2, 2)
	if err != nil {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	allowed := []string{}
	switch action {
	case "remove":
		allowed = []string{"--step"}
	}
	if err := ctx.flags.rejectValueFlagsExcept(allowed...); err != nil {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	plan, err := readPlanForEdit(ctx.sourcePath)
	if err != nil {
		return reportEditError(ctx, "implementation", err)
	}
	steps := append([]Step(nil), plan.Implementation...)
	switch action {
	case "remove":
		if err := rejectDiffStdin(ctx); err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		if len(steps) == 1 {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "cannot remove the final implementation step"))
			return 2
		}
		idx, err := ctx.flags.index("--step")
		if err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		if idx > len(steps) {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--step %d out of range", idx)))
			return 2
		}
		steps = append(steps[:idx-1], steps[idx:]...)
		return runEditPreview(ctx, ReplaceOptions{Section: "implementation"}, jsonBytes(steps))
	default:
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "usage: planner implementation step remove|title set|summary set|file-change ..."))
		return 2
	}
}

func runFileChangeEdit(ctx editContext) int {
	pos := ctx.flags.positional
	if len(pos) < 3 || pos[0] != "step" || pos[1] != "file-change" {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "usage: planner implementation step file-change ..."))
		return 2
	}
	action := pos[2]
	if action == "filename" || action == "explanation" {
		var text []string
		var err error
		maxTail := 3
		ctx, text, err = requirePositional(ctx, []string{"step", "file-change", action, "set"}, 2, maxTail)
		if err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		return mutateFileChange(ctx, action, text)
	}
	if err := rejectStdinForStructured(ctx); err != nil {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	var err error
	ctx, _, err = requirePositional(ctx, []string{"step", "file-change", action}, 2, 2)
	if err != nil {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	allowed := []string{}
	switch action {
	case "remove":
		allowed = []string{"--step", "--change"}
	}
	if err := ctx.flags.rejectValueFlagsExcept(allowed...); err != nil {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	plan, err := readPlanForEdit(ctx.sourcePath)
	if err != nil {
		return reportEditError(ctx, "implementation", err)
	}
	step, updated, ok := selectedStep(ctx, plan)
	if !ok {
		return 2
	}
	switch action {
	case "remove":
		if err := rejectDiffStdin(ctx); err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		if len(updated.FileChanges) == 1 {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "cannot remove the final file change from a step"))
			return 2
		}
		change, err := ctx.flags.index("--change")
		if err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		if change > len(updated.FileChanges) {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--change %d out of range", change)))
			return 2
		}
		updated.FileChanges = append(updated.FileChanges[:change-1], updated.FileChanges[change:]...)
	default:
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, "usage: planner implementation step file-change remove|filename set|explanation set"))
		return 2
	}
	return runEditPreview(ctx, ReplaceOptions{Section: "implementation", Subsection: fmt.Sprint(step)}, jsonBytes(updated))
}

func mutateFileChange(ctx editContext, action string, text []string) int {
	if err := ctx.flags.rejectValueFlagsExcept("--step", "--change"); err != nil {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if err := rejectDiffStdin(ctx); err != nil {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	plan, err := readPlanForEdit(ctx.sourcePath)
	if err != nil {
		return reportEditError(ctx, "implementation", err)
	}
	step, updated, ok := selectedStep(ctx, plan)
	if !ok {
		return 2
	}
	change, err := ctx.flags.index("--change")
	if err != nil {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if change > len(updated.FileChanges) {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--change %d out of range", change)))
		return 2
	}
	fc := updated.FileChanges[change-1]
	switch action {
	case "filename":
		v, err := scalarValue(ctx, text, true)
		if err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		fc.Filename = string(v)
	case "explanation":
		v, err := scalarValue(ctx, text, true)
		if err != nil {
			reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
			return 2
		}
		fc.Explanation = string(v)
	}
	updated.FileChanges[change-1] = fc
	return runEditPreview(ctx, ReplaceOptions{Section: "implementation", Subsection: fmt.Sprint(step)}, jsonBytes(updated))
}

func selectedStep(ctx editContext, plan Plan) (int, Step, bool) {
	step, err := ctx.flags.index("--step")
	if err != nil {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 0, Step{}, false
	}
	if step > len(plan.Implementation) {
		reportError(ctx.stderr, "implementation", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("--step %d out of range", step)))
		return 0, Step{}, false
	}
	return step, plan.Implementation[step-1], true
}
