package internal

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"sort"
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

type editContext struct {
	cmd        string
	sourcePath string
	outputPath string
	flags      editFlagSet
	stdout     io.Writer
	stderr     io.Writer
}

func runBehavioralEdit(args []string, stdout, stderr io.Writer) int {
	cmdName := args[0]
	fs, err := parseEditFlags(args[1:])
	if err != nil {
		reportError(stderr, cmdName, newPlannerCLIError(PlannerUsageError, err, err.Error()))
		return 2
	}
	ctx := editContext{cmd: cmdName, flags: fs, stdout: stdout, stderr: stderr}
	switch args[0] {
	case "title":
		return runTitleEdit(ctx)
	case "overview":
		return runOverviewEdit(ctx)
	case "dod":
		return runDoDEdit(ctx)
	case "implementation":
		return runImplementationEdit(ctx)
	case "verification":
		return runVerificationEdit(ctx)
	default:
		reportError(stderr, "planner", newPlannerCLIError(PlannerUsageError, nil, fmt.Sprintf("unknown command: %s", args[0])))
		return 2
	}
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
				if fs.seen[a] {
					return fs, fmt.Errorf("duplicate flag %s", a)
				}
				if i+1 >= len(args) || strings.HasPrefix(args[i+1], "--") {
					return fs, fmt.Errorf("missing value for %s", a)
				}
				v := args[i+1]
				if strings.TrimSpace(v) == "" {
					return fs, fmt.Errorf("%s must not be whitespace-only", a)
				}
				fs.seen[a] = true
				fs.values[a] = v
				i++
			} else {
				fs.positional = append(fs.positional, a)
			}
		}
	}
	if fs.stdin && fs.diffStdin {
		return fs, fmt.Errorf("--stdin and --diff-stdin are mutually exclusive")
	}
	return fs, nil
}

func (f editFlagSet) rejectValueFlagsExcept(allowed ...string) error {
	allow := map[string]bool{}
	for _, name := range allowed {
		allow[name] = true
	}
	names := make([]string, 0, len(f.values))
	for name := range f.values {
		names = append(names, name)
	}
	sort.Strings(names)
	for _, name := range names {
		if !allow[name] {
			return fmt.Errorf("unknown flag %s", name)
		}
	}
	return nil
}

func (f editFlagSet) stringFlag(names ...string) (string, error) {
	for _, n := range names {
		if v, ok := f.values[n]; ok {
			return v, nil
		}
	}
	return "", fmt.Errorf("missing required flag %s", names[0])
}

func (f editFlagSet) index(names ...string) (int, error) {
	s, err := f.stringFlag(names...)
	if err != nil {
		return 0, err
	}
	n, err := strconv.Atoi(s)
	if err != nil || n < 1 {
		return 0, fmt.Errorf("%s must be a 1-based integer", names[0])
	}
	return n, nil
}

func requirePositional(ctx editContext, prefix []string, minTail, maxTail int) (editContext, []string, error) {
	pos := ctx.flags.positional
	if len(pos) < len(prefix)+minTail || (maxTail >= 0 && len(pos) > len(prefix)+maxTail) {
		return ctx, nil, fmt.Errorf("usage: planner %s", strings.Join(append([]string{ctx.cmd}, prefix...), " "))
	}
	for i, want := range prefix {
		if pos[i] != want {
			return ctx, nil, fmt.Errorf("usage: planner %s", strings.Join(append([]string{ctx.cmd}, prefix...), " "))
		}
	}
	tail := pos[len(prefix):]
	ctx.sourcePath = tail[0]
	ctx.outputPath = tail[1]
	return ctx, tail[2:], nil
}

func scalarValue(ctx editContext, positional []string, required bool) ([]byte, error) {
	if ctx.flags.diffStdin {
		return nil, fmt.Errorf("--diff-stdin is only valid for structured add commands")
	}
	if ctx.flags.stdin {
		if len(positional) > 0 {
			return nil, fmt.Errorf("--stdin cannot be used with positional text")
		}
		b, err := readRawScalar("", true)
		if err != nil {
			return nil, err
		}
		if required && strings.TrimSpace(string(b)) == "" {
			return nil, fmt.Errorf("value must not be whitespace-only")
		}
		return b, nil
	}
	if len(positional) == 0 {
		if required {
			return nil, fmt.Errorf("missing required text")
		}
		return []byte(""), nil
	}
	if strings.TrimSpace(positional[0]) == "" {
		return nil, fmt.Errorf("value must not be whitespace-only")
	}
	return []byte(positional[0]), nil
}

func runEditPreview(ctx editContext, opts ReplaceOptions, patch []byte) int {
	out, _, err := PreviewFromData(ctx.sourcePath, opts, patch)
	if err != nil {
		cliErr := mapReplaceCLIError(err, ctx.sourcePath)
		reportError(ctx.stderr, ctx.cmd, cliErr)
		return plannerExitCode(cliErr)
	}
	exit := runPreviewAgainstSource(ctx.stdout, ctx.stderr, ctx.flags.preview, out, ctx.sourcePath, ctx.outputPath, ctx.cmd, func() error {
		if err := WriteAtomic(ctx.outputPath, []byte(out)); err != nil {
			return newPlannerCLIError(PlannerWriteOutputError, err, ctx.outputPath)
		}
		return nil
	})
	return exit
}

func readPlanForEdit(path string) (Plan, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return Plan{}, newPlannerCLIError(PlannerReadInputError, err, path)
	}
	p, err := ParseMarkdown(string(raw))
	if err != nil {
		return Plan{}, plannerMarkdownDecodeError(raw, err)
	}
	return p.Plan, nil
}

func reportEditError(ctx editContext, cmd string, err error) int {
	reportError(ctx.stderr, cmd, err)
	return plannerExitCode(err)
}

func jsonBytes(v any) []byte {
	b, _ := json.Marshal(v)
	return b
}

func rejectDiffStdin(ctx editContext) error {
	if ctx.flags.diffStdin {
		return fmt.Errorf("--diff-stdin is only valid for structured add commands")
	}
	return nil
}

func rejectStdinForStructured(ctx editContext) error {
	if ctx.flags.stdin {
		return fmt.Errorf("--stdin is only valid for scalar and diff set commands")
	}
	return nil
}
