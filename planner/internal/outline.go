package internal

import (
	"errors"
	"fmt"
	"io"
	"os"
	"strings"
)

// One heading or file change of a plan and the lines it covers.
type outlineEntry struct {
	// 2 for a ## heading, 3 for a ### heading and 4 for a file change.
	Level int
	// The line as written, such as "### 1. Add the greeting".
	Text string
	// Line numbers count from 1, and End is the part's last line.
	Start int
	End   int
}

func runOutline(
	args []string,
	stdout io.Writer,
	stderr io.Writer,
) int {
	path, _, err := parseGuardedArgs(
		args,
		"",
		"",
	)
	if err != nil {
		reportError(
			stderr,
			"outline",
			newPlannerCLIError(
				PlannerUsageError,
				err,
				err.Error()+"; usage: planner outline <plan.md>",
			),
		)
		return 2
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		reportError(
			stderr,
			"outline",
			newPlannerCLIError(
				PlannerReadInputError,
				err,
				path,
			),
		)
		return 1
	}
	entries, err := outlinePlan(string(raw))
	if err != nil {
		reportError(
			stderr,
			"outline",
			newPlannerCLIError(
				PlannerValidateInputError,
				err,
				path,
			),
		)
		return 1
	}
	for _, entry := range entries {
		_, _ = fmt.Fprintf(
			stdout,
			"%-9s %s%s\n",
			fmt.Sprintf(
				"%d-%d",
				entry.Start,
				entry.End,
			),
			strings.Repeat(
				"  ",
				entry.Level-2,
			),
			entry.Text,
		)
	}
	return 0
}

func runShow(
	args []string,
	stdout io.Writer,
	stderr io.Writer,
) int {
	const usage = "usage: planner show <plan.md> (--step N | --section NAME)"
	path, opts, err := parseGuardedArgs(
		args,
		"--step --section",
		"",
	)
	if err == nil && (opts["--step"] == "") == (opts["--section"] == "") {
		err = errors.New("pass exactly one of --step and --section")
	}
	if err != nil {
		reportError(
			stderr,
			"show",
			newPlannerCLIError(
				PlannerUsageError,
				err,
				err.Error()+"; "+usage,
			),
		)
		return 2
	}
	raw, err := os.ReadFile(path)
	if err != nil {
		reportError(
			stderr,
			"show",
			newPlannerCLIError(
				PlannerReadInputError,
				err,
				path,
			),
		)
		return 1
	}
	entries, err := outlinePlan(string(raw))
	if err != nil {
		reportError(
			stderr,
			"show",
			newPlannerCLIError(
				PlannerValidateInputError,
				err,
				path,
			),
		)
		return 1
	}
	entry, err := findPlanPart(
		entries,
		opts["--step"],
		opts["--section"],
	)
	if err != nil {
		reportError(
			stderr,
			"show",
			&PlannerCLIError{
				Code:         PlannerValidateInputError,
				Message:      err.Error(),
				RecoveryHint: "run planner outline to list the plan's steps and sections",
			},
		)
		return 1
	}
	lines := strings.SplitAfter(
		string(raw),
		"\n",
	)
	_, _ = io.WriteString(
		stdout,
		strings.Join(
			lines[entry.Start-1:entry.End],
			"",
		),
	)
	return 0
}

// Returns each ## and ### heading and each file change outside code fences,
// in file order. A part ends where the next part of its level or higher starts.
func outlinePlan(input string) ([]outlineEntry, error) {
	lines := strings.SplitAfter(
		input,
		"\n",
	)
	if lines[len(lines)-1] == "" {
		lines = lines[:len(lines)-1]
	}
	entries := []outlineEntry{}
	fence := ""
	fenceLine := 0
	for i, raw := range lines {
		line := strings.TrimSuffix(
			raw,
			"\n",
		)
		if fence != "" {
			// A line of only backticks, at least as many as opened the fence, closes it.
			trimmed := strings.TrimSpace(line)
			onlyTicks := strings.Trim(
				trimmed,
				"`",
			) == ""
			if onlyTicks && len(trimmed) >= len(fence) {
				fence = ""
			}
			continue
		}
		if opening, ok := parseFence(line); ok {
			fence = opening
			fenceLine = i + 1
			continue
		}
		level := 0
		switch {
		case strings.HasPrefix(
			line,
			"## ",
		):
			level = 2
		case strings.HasPrefix(
			line,
			"### ",
		):
			level = 3
		case isFileChangeHeader(
			lines,
			i,
		):
			level = 4
		default:
			continue
		}
		entries = append(entries, outlineEntry{
			Level: level,
			Text:  line,
			Start: i + 1,
		})
	}
	// An unclosed block would hide every later heading without an error.
	if fence != "" {
		return nil, fmt.Errorf(
			"the code block opened on line %d never closes",
			fenceLine,
		)
	}
	for i := range entries {
		entries[i].End = len(lines)
		for _, next := range entries[i+1:] {
			if next.Level <= entries[i].Level {
				entries[i].End = next.Start - 1
				break
			}
		}
	}
	return entries, nil
}

// Returns the first ### heading under ## Implementation that starts with the
// step number, or the one ## or ### heading whose text matches section in any
// letter case.
func findPlanPart(
	entries []outlineEntry,
	step string,
	section string,
) (outlineEntry, error) {
	if step != "" {
		inImplementation := false
		for _, entry := range entries {
			if entry.Level == 2 {
				inImplementation = entry.Text == "## Implementation"
			}
			if inImplementation && entry.Level == 3 && strings.HasPrefix(
				entry.Text,
				"### "+step+". ",
			) {
				return entry, nil
			}
		}
		return outlineEntry{}, fmt.Errorf(
			"the plan has no step %s",
			step,
		)
	}
	matches := []outlineEntry{}
	for _, entry := range entries {
		text := strings.TrimLeft(
			entry.Text,
			"# ",
		)
		if entry.Level < 4 && strings.EqualFold(
			text,
			section,
		) {
			matches = append(
				matches,
				entry,
			)
		}
	}
	switch len(matches) {
	case 0:
		return outlineEntry{}, fmt.Errorf(
			"the plan has no section %q",
			section,
		)
	case 1:
		return matches[0], nil
	default:
		return outlineEntry{}, fmt.Errorf(
			"%d sections are named %q; read one by its lines from planner outline",
			len(matches),
			section,
		)
	}
}
