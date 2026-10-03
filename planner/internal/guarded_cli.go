package internal

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"strings"

	"planner/internal/planpatch"
)

const guardedHelp = `
Guarded source-code revisions:
  planner inspect <plan.md> --target SELECTOR --repo DIR --base COMMIT
      [--code-out NEWFILE [--before]] [--json-errors]
  planner patch <plan.md> --target SELECTOR --expect TOKEN --repo DIR --base COMMIT
      (--after-file FILE | --diff-file FILE) [--dry-run] [--diff] [--json-errors]
  planner check <plan.md> [--repo DIR] [--base COMMIT] [--json-errors]

  SELECTOR is implementation[N].file_changes[M], with 1-based indices. Leading
  zeros are accepted and the normalized selector is echoed in results.
  inspect and patch require --repo and --base. check validates structure, then
  starts with the original code version recorded in the plan and tries every
  planned change in order. It does not run tests or check behavior. --base is
  the full commit ID recorded in the plan's Current State when the plan was
  created, not the current HEAD of a worktree. Without --base, check reads the
  first line of Current State, "Baseline commit: <full commit ID>".
  Without --repo, check uses the current working directory's Git repository.
  Dirty and untracked source files are excluded; Planner does not stage, stash,
  reset, or commit them.

  --target SELECTOR               1-based implementation step and file change.
  --expect TOKEN                  edit_expect from targeted inspect.
  --code-out NEWFILE              Write proposed source to a NEW scratch file.
  --before                        Export source before the selected change.
  --after-file FILE               Generate a diff from ordinary source; use
                                  /dev/null to propose deleting the file.
  --diff-file FILE                Import a raw unified diff; use - for stdin.
  --dry-run                       Validate without writing the plan.
  --diff                          Print a Git-generated review preview.

  Without --code-out, inspect returns JSON with the selected diff and edit_expect.
  With --code-out, inspect writes the file after the selected change, or before
  it with --before, to a new file.
  patch starts with the original code version and tries every change through the
  edited one, never later changes, and reports prefix_replayed: true with
  downstream_checked: false. Run planner check for whole-plan readiness.
  --after-file retains an existing file's mode and defaults a new file to 100644.
  edit_expect binds the plan bytes, the normalized selector, and the base, so
  any edit to the plan invalidates it.
`

// hasArg reports whether an exact argument appears. Guarded routing uses the
// presence of new flags, so old command grammar keeps its existing behavior.
func hasArg(args []string, flag string) bool {
	for _, arg := range args {
		if arg == flag {
			return true
		}
	}
	return false
}

// parseGuardedArgs accepts flags before or after the one required plan path.
// Each caller supplies its accepted flags so misspelled options fail closed.
func parseGuardedArgs(args []string, values, switches string) (string, map[string]string, error) {
	allowedValues, allowedSwitches := map[string]bool{}, map[string]bool{}
	for _, flag := range strings.Fields(values) {
		allowedValues[flag] = true
	}
	for _, flag := range strings.Fields(switches) {
		allowedSwitches[flag] = true
	}
	opts := map[string]string{}
	var positional []string
	for i := 0; i < len(args); i++ {
		arg := args[i]
		if allowedValues[arg] || allowedSwitches[arg] {
			if _, seen := opts[arg]; seen {
				return "", nil, fmt.Errorf("duplicate flag %s", arg)
			}
			opts[arg] = "true"
			if allowedValues[arg] {
				i++
				if i == len(args) || args[i] == "" || strings.HasPrefix(args[i], "--") {
					return "", nil, fmt.Errorf("%s requires a value", arg)
				}
				opts[arg] = args[i]
			}
		} else if strings.HasPrefix(arg, "-") {
			return "", nil, fmt.Errorf("unknown flag %s", arg)
		} else {
			positional = append(positional, arg)
		}
	}
	if len(positional) != 1 {
		return "", nil, fmt.Errorf("exactly one plan path is required")
	}
	return positional[0], opts, nil
}

// guardedFailure is the emitter for guarded operations. It shares the legacy
// {code, message, recovery_hint} JSON shape and exit codes but is separate
// because the code may come from a planpatch.Error rather than a PlannerCLIError.
// When the operation carried a planpatch code, that code wins and the message
// omits the duplicate prefix because planpatch.Error reports only its cause.
func guardedFailure(stderr io.Writer, code string, err error) int {
	message := err.Error()
	hint := "Fix the identified input or source assumption; do not retry unchanged."
	// Markdown decode failures carry a subject and, for a malformed wrapper, a
	// recovery hint. Honor both so check matches the other read commands.
	var cliErr *PlannerCLIError
	if errors.As(err, &cliErr) {
		code = plannerErrorCodeNames[cliErr.Code]
		if cliErr.RecoveryHint != "" {
			hint = cliErr.RecoveryHint
		}
	}
	var patchErr *planpatch.Error
	if errors.As(err, &patchErr) {
		code = patchErr.Code
	}
	switch code {
	case planpatch.CodePlanStale:
		hint = "Reread the plan and reconcile changes. Never refresh only the " +
			"token and retry an old replacement."
	case planpatch.CodePlanBusy:
		hint = "Wait for the active writer. Remove a leftover lock only after " +
			"confirming no writer is active."
	case codeBaselineRequired:
		hint = `Record "Baseline commit: <full commit ID>" as the first line of ` +
			"Current State or pass --base."
	}
	if jsonErrorOutput {
		_ = json.NewEncoder(stderr).Encode(struct {
			Code         string `json:"code"`
			Message      string `json:"message"`
			RecoveryHint string `json:"recovery_hint"`
		}{Code: code, Message: message, RecoveryHint: hint})
	} else {
		_, _ = fmt.Fprintf(stderr, "planner: %s: %s\n", code, message)
	}
	if code == plannerCode(PlannerUsageError) {
		return 2
	}
	return 1
}

func runGuardedInspect(args []string, stdout, stderr io.Writer) int {
	name, opts, err := parseGuardedArgs(args,
		"--target --repo --base --code-out", "--before")
	if err != nil {
		return guardedFailure(stderr, plannerCode(PlannerUsageError), err)
	}
	result, err := guardedInspect(guardedInspectOptions{
		PlanPath: name,
		Target:   opts["--target"],
		Repo:     opts["--repo"],
		Base:     opts["--base"],
		CodeOut:  opts["--code-out"],
		Before:   opts["--before"] != "",
	})
	if err != nil {
		return guardedFailure(stderr, codeSourceCheck, err)
	}
	if err := json.NewEncoder(stdout).Encode(result); err != nil {
		return guardedFailure(stderr, codeOutputReportFailed, err)
	}
	return 0
}

func runGuardedPatch(args []string, stdout, stderr io.Writer) int {
	name, opts, err := parseGuardedArgs(args,
		"--target --expect --after-file --diff-file --repo --base",
		"--dry-run --diff")
	if err != nil {
		return guardedFailure(stderr, plannerCode(PlannerUsageError), err)
	}
	result, err := guardedPatch(guardedPatchOptions{
		PlanPath:  name,
		Target:    opts["--target"],
		Expect:    opts["--expect"],
		AfterFile: opts["--after-file"],
		DiffFile:  opts["--diff-file"],
		Repo:      opts["--repo"],
		Base:      opts["--base"],
		DryRun:    opts["--dry-run"] != "",
		Diff:      opts["--diff"] != "",
	})
	if err != nil {
		return guardedFailure(stderr, codePatchInput, err)
	}
	if opts["--diff"] != "" {
		_, err = stdout.Write(result.Preview)
	} else {
		err = json.NewEncoder(stdout).Encode(result)
	}
	// A broken stdout can occur after a successful write. Do not tell the caller
	// the plan is unchanged; they must inspect it before retrying.
	if err != nil {
		return guardedFailure(stderr, codeOutputReportFailed, fmt.Errorf(
			"result reporting failed; plan may already be written: %w", err))
	}
	return 0
}

func runGuardedCheck(args []string, stdout, stderr io.Writer) int {
	const usage = "usage: planner check <plan.md> [--repo DIR] [--base COMMIT] [--json-errors]"
	name, opts, err := parseGuardedArgs(args, "--repo --base", "")
	if err != nil {
		return guardedFailure(stderr, plannerCode(PlannerUsageError), err)
	}
	// check still reads Markdown only. Reject a JSON plan path with the usage
	// error the old plain check gave, rather than a confusing decode failure.
	if strings.HasSuffix(strings.ToLower(name), ".json") {
		return guardedFailure(stderr, plannerCode(PlannerUsageError),
			fmt.Errorf("planner check no longer accepts JSON plan input: %s", usage))
	}
	result, err := guardedCheck(guardedCheckOptions{
		PlanPath: name,
		Repo:     opts["--repo"],
		Base:     opts["--base"],
	})
	if err != nil {
		return guardedFailure(stderr, codeSourceCheck, err)
	}
	if err := json.NewEncoder(stdout).Encode(result); err != nil {
		return guardedFailure(stderr, codeOutputReportFailed, err)
	}
	return 0
}
