package internal

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

// Letters, a hyphen, digits and optional letters at the start of a file name,
// as in "ABC-12 Approvals file.md" or "XYZ-07B Refactor.md".
var planKeyPattern = regexp.MustCompile(`^([A-Za-z]+-[0-9]+[A-Za-z]*)`)

func runKey(
	args []string,
	stdout io.Writer,
	stderr io.Writer,
) int {
	if len(args) != 1 {
		reportError(
			stderr,
			"key",
			newPlannerCLIError(
				PlannerUsageError,
				nil,
				"usage: planner key <plan.md>",
			),
		)
		return 2
	}
	path := args[0]
	if _, err := os.Stat(path); err != nil {
		reportError(
			stderr,
			"key",
			newPlannerCLIError(
				PlannerReadInputError,
				err,
				path,
			),
		)
		return 1
	}

	name := filepath.Base(path)
	match := planKeyPattern.FindStringSubmatch(name)
	if match == nil {
		reportError(
			stderr,
			"key",
			&PlannerCLIError{
				Code: PlannerValidateInputError,
				Message: fmt.Sprintf(
					"%s does not start with an issue key, such as ABC-12",
					name,
				),
				RecoveryHint: "pass the plan's issue file; design docs and notes have no key",
			},
		)
		return 1
	}
	_, _ = io.WriteString(
		stdout,
		strings.ToLower(match[1])+"\n",
	)
	return 0
}
