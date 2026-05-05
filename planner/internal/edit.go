package internal

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"strconv"
	"strings"
)

type editFlagSet struct {
	positional []string
	values     map[string]string
	seen       map[string]bool
	stdin      bool
	diffStdin  bool
	preview    previewFlags
}

func runBehavioralEdit(args []string, stdout, stderr io.Writer) int {
	if len(args) == 0 {
		reportError(stderr, "planner", newPlannerCLIError(PlannerUsageError, nil, "missing edit command"))
		return 2
	}
	cmdName := args[0]
	fs, err := parseEditFlags(args[1:])
	if err != nil {
		reportError(stderr, cmdName, newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	if len(fs.positional) < 2 {
		reportError(stderr, cmdName, newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("usage: planner %s ... <plan.md> <output.md>", strings.Join(args[:min(len(args), 3)], " "))))
		return 2
	}
	ctx := editContext{cmd: cmdName, sourcePath: fs.positional[len(fs.positional)-2], outputPath: fs.positional[len(fs.positional)-1], flags: fs, stdout: stdout, stderr: stderr}
	switch args[0] {
	case "title":
		return runTitleEdit(ctx, args[1:])
	case "overview":
		return runOverviewEdit(ctx, args[1:])
	case "dod":
		return runDoDEdit(ctx, args[1:])
	case "implementation":
		return runImplementationEdit(ctx, args[1:])
	case "verification":
		return runVerificationEdit(ctx, args[1:])
	default:
		reportError(stderr, "planner", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("unknown command: %s", args[0])))
		return 2
	}
}

type editContext struct {
	cmd        string
	sourcePath string
	outputPath string
	flags      editFlagSet
	stdout     io.Writer
	stderr     io.Writer
}

func parseEditFlags(args []string) (editFlagSet, error) {
	fs := editFlagSet{values: map[string]string{}, seen: map[string]bool{}}
	for i := 0; i < len(args); i++ {
		a := args[i]
		switch a {
		case "--stdin":
			fs.stdin = true
		case "--diff-stdin":
			fs.diffStdin = true
		case "--diff":
			fs.preview.diff = true
		case "--dry-run":
			fs.preview.dryRun = true
		default:
			if strings.HasPrefix(a, "--") {
				if fs.seen[a] { return fs, fmt.Errorf("duplicate flag %s", a) }
				if i+1 >= len(args) || strings.HasPrefix(args[i+1], "--") { return fs, fmt.Errorf("missing value for %s", a) }
				v := args[i+1]
				if strings.TrimSpace(v) == "" { return fs, fmt.Errorf("%s must not be whitespace-only", a) }
				fs.seen[a] = true; fs.values[a] = v; i++
			} else { fs.positional = append(fs.positional, a) }
		}
	}
	if fs.stdin && fs.diffStdin { return fs, fmt.Errorf("--stdin and --diff-stdin are mutually exclusive") }
	return fs, nil
}

func (f editFlagSet) stringFlag(names ...string) (string, error) {
	for _, n := range names { if v, ok := f.values[n]; ok { return v, nil } }
	return "", fmt.Errorf("missing required flag %s", names[0])
}
func (f editFlagSet) optional(names ...string) string { v, _ := f.stringFlag(names...); return v }
func (f editFlagSet) index(names ...string) (int, error) { s, err := f.stringFlag(names...); if err != nil { return 0, err }; n, err := strconv.Atoi(s); if err != nil || n < 1 { return 0, fmt.Errorf("%s must be a 1-based integer", names[0]) }; return n, nil }

func scalarValue(ctx editContext, names ...string) ([]byte, error) {
	if ctx.flags.stdin { b, err := readRawScalar("", true); if err != nil { return nil, err }; if strings.TrimSpace(string(b)) == "" { return nil, fmt.Errorf("value must not be whitespace-only") }; return b, nil }
	v, err := ctx.flags.stringFlag(names...); if err != nil { return nil, err }
	return []byte(v), nil
}

func runEditPreview(ctx editContext, opts ReplaceOptions, patch []byte) int {
	out, _, err := PreviewFromData(ctx.sourcePath, opts, patch)
	if err != nil { cliErr := mapReplaceCLIError(err, ctx.sourcePath); reportError(ctx.stderr, ctx.cmd, cliErr); return plannerExitCode(cliErr) }
	exit := runPreviewAgainstSource(ctx.stdout, ctx.stderr, ctx.flags.preview, out, ctx.sourcePath, ctx.outputPath, ctx.cmd, func() error { if err := WriteAtomic(ctx.outputPath, []byte(out)); err != nil { return newPlannerCLIError(PlannerWriteOutputError, err, ctx.outputPath) }; return nil })
	return exit
}

func readPlanForEdit(path string) (Plan, error) { raw, err := os.ReadFile(path); if err != nil { return Plan{}, err }; p, err := ParseMarkdown(string(raw)); if err != nil { return Plan{}, err }; return p.Plan, nil }
func mustJSON(v any) []byte { b, _ := json.Marshal(v); return b }
func rejectDiffStdin(ctx editContext) error { if ctx.flags.diffStdin { return fmt.Errorf("--diff-stdin is only valid for structured add commands") }; return nil }
func rejectStdinForStructured(ctx editContext) error { if ctx.flags.stdin { return fmt.Errorf("--stdin is only valid for scalar and diff set commands") }; return nil }
func min(a,b int) int { if a < b { return a }; return b }
