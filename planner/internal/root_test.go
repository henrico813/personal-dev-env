package internal

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestHelpTextIncludesRules(t *testing.T) {
	help := buildHelpText()

	// Positive anchors: every command we ship must appear in help so AIs can
	// discover the current surface from `planner help` alone.
	for _, command := range []string{
		"planner new",
		"planner check",
		"planner inspect",
		"planner patch",
	} {
		if !strings.Contains(help, command) {
			t.Fatalf("buildHelpText() missing command %q", command)
		}
	}
	if strings.Contains(help, "planner validate") {
		t.Fatal("buildHelpText() must not mention planner validate")
	}

	// Negative anchors: deleted commands and removed flags must not reappear.
	for _, banned := range []string{"show-schema", "planner generate", "planner replace", "--write"} {
		if strings.Contains(help, banned) {
			t.Fatalf("buildHelpText() still mentions removed token %q", banned)
		}
	}
}

func TestHelpTextMentionsMarkdownFirstFlow(t *testing.T) {
	help := buildHelpText()
	for _, want := range []string{
		"planner new plan.md.",
		"planner check plan.md --json-errors.",
		"<out.md> may be the same path as <plan.md>",
		"planner patch <plan.md> [<out.md>]",
		"planner patch preserves wrapped frontmatter",
		"same-file updates",
	} {
		if !strings.Contains(help, want) {
			t.Fatalf("buildHelpText() missing %q", want)
		}
	}
}

func TestReadRawScalarStripsTrailingNewline(t *testing.T) {
	t.Run("stdin_lf", func(t *testing.T) {
		withStdin(t, []byte("raw text\n"), func() {
			got, err := readRawScalar("", true)
			if err != nil {
				t.Fatalf("readRawScalar(stdin): %v", err)
			}
			if string(got) != "raw text" {
				t.Fatalf("readRawScalar(stdin) = %q, want %q", got, "raw text")
			}
		})
	})
	t.Run("file_crlf", func(t *testing.T) {
		dir := t.TempDir()
		path := dir + "/raw.txt"
		if err := os.WriteFile(path, []byte("raw text\r\n"), 0o644); err != nil {
			t.Fatal(err)
		}
		got, err := readRawScalar(path, false)
		if err != nil {
			t.Fatalf("readRawScalar(file): %v", err)
		}
		if string(got) != "raw text" {
			t.Fatalf("readRawScalar(file) = %q, want %q", got, "raw text")
		}
	})
	t.Run("file_double_lf", func(t *testing.T) {
		dir := t.TempDir()
		path := dir + "/raw.txt"
		if err := os.WriteFile(path, []byte("raw text\n\n"), 0o644); err != nil {
			t.Fatal(err)
		}
		got, err := readRawScalar(path, false)
		if err != nil {
			t.Fatalf("readRawScalar(file): %v", err)
		}
		if string(got) != "raw text\n" {
			t.Fatalf("readRawScalar(file) = %q, want %q", got, "raw text\n")
		}
	})
}

func TestNewRejectsNonMarkdownOutput(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/plan.txt"
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exit := Execute([]string{"new", out}, &stdout, &stderr); exit != 2 {
		t.Fatalf("Execute(new) exit = %d, want 2; stderr = %q", exit, stderr.String())
	}
	if !strings.Contains(stderr.String(), "planner new requires an output path ending in .md") {
		t.Fatalf("stderr %q missing .md requirement", stderr.String())
	}
}

func TestNewJSONErrorsReportsUsage(t *testing.T) {
	dir := t.TempDir()
	badOut := dir + "/plan.txt"
	cases := []struct {
		name string
		args []string
		want string
	}{
		{name: "non_md", args: []string{"--json-errors", "new", badOut}, want: "planner new requires an output path ending in .md: usage: planner new <output.md> [--diff] [--dry-run] [--json-errors]"},
		{name: "missing_output", args: []string{"new", "--json-errors"}, want: "usage: planner new <output.md> [--diff] [--dry-run] [--json-errors]"},
		{name: "extra_output", args: []string{"new", badOut, "extra", "--json-errors"}, want: "usage: planner new <output.md> [--diff] [--dry-run] [--json-errors]"},
	}
	for _, tc := range cases {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if exit := Execute(tc.args, &stdout, &stderr); exit != 2 {
				t.Fatalf("Execute(%v) exit = %d, want 2; stderr = %q", tc.args, exit, stderr.String())
			}
			code, msg := firstStderrJSON(t, &stderr)
			if code != "USAGE" {
				t.Fatalf("code=%q want USAGE", code)
			}
			if !strings.Contains(msg, tc.want) {
				t.Fatalf("message %q missing %q", msg, tc.want)
			}
		})
	}
}

func TestNewMatchesScaffoldHelper(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/plan.md"

	var newStdout bytes.Buffer
	var newStderr bytes.Buffer
	if exit := Execute([]string{"new", out}, &newStdout, &newStderr); exit != 0 {
		t.Fatalf("Execute(new) exit = %d, stderr = %q", exit, newStderr.String())
	}
	got, err := os.ReadFile(out)
	if err != nil {
		t.Fatalf("ReadFile(%s): %v", out, err)
	}
	want, err := renderCanonicalScaffold()
	if err != nil {
		t.Fatalf("renderCanonicalScaffold: %v", err)
	}
	if string(got) != want {
		t.Fatalf("new scaffold mismatch")
	}
}

func TestNewScaffoldPassesCheckAndInspect(t *testing.T) {
	path := writeNewScaffold(t, t.TempDir())

	var checkStdout bytes.Buffer
	var checkStderr bytes.Buffer
	if exit := Execute([]string{"check", path}, &checkStdout, &checkStderr); exit != 0 {
		t.Fatalf("Execute(check) exit = %d, stderr = %q", exit, checkStderr.String())
	}

	var inspectStdout bytes.Buffer
	var inspectStderr bytes.Buffer
	if exit := Execute([]string{"inspect", path}, &inspectStdout, &inspectStderr); exit != 0 {
		t.Fatalf("Execute(inspect) exit = %d, stderr = %q", exit, inspectStderr.String())
	}
	plan, err := DecodePlan(inspectStdout.Bytes())
	if err != nil {
		t.Fatalf("inspect output is not valid plan JSON: %v", err)
	}
	if err := ValidatePlan(plan); err != nil {
		t.Fatalf("inspect output does not validate: %v", err)
	}
}

func TestNewScaffoldSupportsSamePathEdits(t *testing.T) {
	cases := []struct {
		name  string
		args  func(string) []string
		check func(*testing.T, Plan)
	}{
		{
			name: "goal_set",
			args: func(path string) []string {
				return []string{"dod", "goal", "set", path, path, "--goal", "1", "updated goal"}
			},
			check: func(t *testing.T, plan Plan) {
				if got := plan.DefinitionOfDone.Goals[0].Text; got != "updated goal" {
					t.Fatalf("goal text = %q, want updated goal", got)
				}
			},
		},
		{
			name: "step_summary_set",
			args: func(path string) []string {
				return []string{"implementation", "step", "summary", "set", path, path, "--step", "1", "updated summary"}
			},
			check: func(t *testing.T, plan Plan) {
				if got := plan.Implementation[0].Summary; got != "updated summary" {
					t.Fatalf("step summary = %q, want updated summary", got)
				}
			},
		},
	}
	for _, tc := range cases {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			path := writeNewScaffold(t, t.TempDir())
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if exit := Execute(tc.args(path), &stdout, &stderr); exit != 0 {
				t.Fatalf("Execute(%v) exit = %d, stderr = %q", tc.args(path), exit, stderr.String())
			}
			assertParsed(t, path, func(plan Plan) { tc.check(t, plan) })
		})
	}
}

func TestNewDryRunDoesNotWriteChanges(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/plan.md"
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exit := Execute([]string{"new", out, "--dry-run"}, &stdout, &stderr); exit != 0 {
		t.Fatalf("Execute(new --dry-run) exit = %d, want 0; stderr = %q", exit, stderr.String())
	}
	if _, err := os.Stat(out); !os.IsNotExist(err) {
		t.Fatalf("output should not be written, stat err = %v", err)
	}
}

func TestNewDryRunDiffDoesNotWriteChanges(t *testing.T) {
	dir := t.TempDir()
	out := dir + "/plan.md"
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exit := Execute([]string{"new", out, "--diff", "--dry-run"}, &stdout, &stderr); exit != 1 {
		t.Fatalf("Execute(new --diff --dry-run) exit = %d, want 1; stderr = %q", exit, stderr.String())
	}
	if stdout.Len() == 0 {
		t.Fatal("expected diff on stdout")
	}
	if _, err := os.Stat(out); !os.IsNotExist(err) {
		t.Fatalf("output should not be written, stat err = %v", err)
	}
}

func TestShowSchemaRemoved(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"show-schema"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("Execute(show-schema) exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "unknown command: show-schema") {
		t.Fatalf("show-schema stderr missing unknown-command message: %q", stderr.String())
	}
}

func TestGenerateRemoved(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"generate"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("Execute(generate) exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "unknown command: generate") {
		t.Fatalf("generate stderr missing unknown-command message: %q", stderr.String())
	}
}

func TestRunInspectUsage(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"inspect"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("Execute(inspect) exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "usage: planner inspect <plan.md>") {
		t.Fatalf("missing inspect usage in stderr = %q", stderr.String())
	}
}

func TestRemovedPublicJSONCommands(t *testing.T) {
	for _, command := range []string{"template", "create"} {
		var stdout bytes.Buffer
		var stderr bytes.Buffer
		if exitCode := Execute([]string{command}, &stdout, &stderr); exitCode != 2 {
			t.Fatalf("Execute(%s) exit code = %d, want 2", command, exitCode)
		}
		if !strings.Contains(stderr.String(), "unknown command: "+command) {
			t.Fatalf("stderr %q missing unknown-command error for %s", stderr.String(), command)
		}
	}
}

func TestJSONErrorsFlagEmitsStructuredJSON(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", "--json-errors", "/no/such/path.md"}, &stdout, &stderr); exit != 1 {
		t.Fatalf("exit %d want 1", exit)
	}
	if strings.Contains(stderr.String(), "planner: reading JSON from stdin") || strings.Contains(stderr.String(), "repaired JSON input") {
		t.Fatalf("unexpected informational stderr in json mode: %q", stderr.String())
	}
	var got struct {
		Code         string `json:"code"`
		Message      string `json:"message"`
		RecoveryHint string `json:"recovery_hint"`
	}
	if err := json.Unmarshal(bytes.TrimSpace(stderr.Bytes()), &got); err != nil {
		t.Fatalf("stderr is not JSON: %v; raw=%q", err, stderr.String())
	}
	if got.Code != "READ_INPUT" {
		t.Fatalf("code=%q want READ_INPUT", got.Code)
	}
	if got.Message == "" {
		t.Fatal("empty message")
	}
}

func TestRunCheckMarkdown(t *testing.T) {
	plan, err := DecodePlan(validPlanJSON())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	dir := t.TempDir()
	path := dir + "/plan.md"
	if err := os.WriteFile(path, []byte(rendered), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", path}, &stdout, &stderr); exit != 0 {
		t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stdout.String(), "OK") {
		t.Fatalf("expected OK in stdout, got %q", stdout.String())
	}
}

func TestRunCheckMarkdownWithCanonicalFrontmatter(t *testing.T) {
	plan, err := DecodePlan(validPlanJSON())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	frontmatter := "---\ntags:\n  - \"#Ticket\"\ntype: issue\nstatus: open\ntemplate_version: 1\nproject: PDEV-083\ndate_created: 2026-05-12\ntopics: []\n---\n\n"
	path := filepath.Join(t.TempDir(), "plan.md")
	if err := os.WriteFile(path, []byte(frontmatter+rendered), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", path}, &stdout, &stderr); exit != 0 {
		t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stdout.String(), "OK") {
		t.Fatalf("expected OK in stdout, got %q", stdout.String())
	}
}

func TestCheckWrappedFrontmatterError(t *testing.T) {
	path := t.TempDir() + "/plan.md"
	bad := strings.Replace(buildPlanWithFrontmatter(t), "\"#Ticket\"", "\"#ticket\"", 1)
	if err := os.WriteFile(path, []byte(bad), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", path}, &stdout, &stderr); exit != 1 {
		t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stderr.String(), "wrapped issue doc markdown") {
		t.Fatalf("stderr missing wrapped-doc subject: %q", stderr.String())
	}
	if !strings.Contains(stderr.String(), "unsupported wrapped issue doc frontmatter") {
		t.Fatalf("stderr missing wrapped-doc error: %q", stderr.String())
	}
}

func TestJSONErrorsWrappedCheck(t *testing.T) {
	path := t.TempDir() + "/plan.md"
	bad := strings.Replace(buildPlanWithFrontmatter(t), "\"#Ticket\"", "\"#ticket\"", 1)
	if err := os.WriteFile(path, []byte(bad), 0o644); err != nil {
		t.Fatal(err)
	}
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", "--json-errors", path}, &stdout, &stderr); exit != 1 {
		t.Fatalf("exit %d want 1; stderr %q", exit, stderr.String())
	}
	code, msg := firstStderrJSON(t, &stderr)
	if code != "DECODE_INPUT" {
		t.Fatalf("code=%q want DECODE_INPUT", code)
	}
	if !strings.Contains(msg, "wrapped issue doc markdown") {
		t.Fatalf("message %q missing wrapped-doc subject", msg)
	}
}

func TestCheckRejectsJSONInputPath(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "plan.json")
	if err := os.WriteFile(path, validPlanJSON(), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	var stdout bytes.Buffer
	var stderr bytes.Buffer
	if exit := Execute([]string{"check", path}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d want 2; stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stderr.String(), "planner check no longer accepts JSON plan input") {
		t.Fatalf("unexpected stderr: %q", stderr.String())
	}
}

func TestValidateCommandRemoved(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"validate"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d want 2; stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stderr.String(), "unknown command: validate") {
		t.Fatalf("unexpected stderr: %q", stderr.String())
	}
}

func TestPatchCommandUsage(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"patch", "--help"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d want 2; stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
	}
	if !strings.Contains(stderr.String(), "usage: planner patch <plan.md> [<out.md>]") {
		t.Fatalf("unexpected stderr: %q", stderr.String())
	}
}

func TestPatchRejectsUnsupportedSelector(t *testing.T) {
	path := writeBehavioralPlan(t, t.TempDir())
	patch := []byte("*** Begin Patch\n*** Update Field: implementation[1].file_changes[1].diff\n-old\n+new\n*** End Patch\n")
	var stdout, stderr bytes.Buffer
	withStdin(t, patch, func() {
		if exit := Execute([]string{"patch", path}, &stdout, &stderr); exit != 1 {
			t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
		}
	})
	if !strings.Contains(stderr.String(), "unsupported patch selector") {
		t.Fatalf("stderr missing unsupported-selector error: %q", stderr.String())
	}
}

func TestBehavioralFallbackStillWorks(t *testing.T) {
	path := writeBehavioralPlan(t, t.TempDir())
	var stdout, stderr bytes.Buffer
	patch := []byte("*** Begin Patch\n*** Update Field: implementation[1].file_changes[1].diff\n-old\n+new\n*** End Patch\n")
	withStdin(t, patch, func() {
		if exit := Execute([]string{"patch", path}, &stdout, &stderr); exit != 1 {
			t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
		}
	})
	runPlannerOK(t, []string{"implementation", "step", "file-change", "diff", "set", path, path, "--step", "1", "--change", "1", "--stdin"}, []byte("raw diff bytes"))
	assertParsed(t, path, func(plan Plan) {
		if plan.Implementation[0].FileChanges[0].Diff != "raw diff bytes" {
			t.Fatalf("diff=%q", plan.Implementation[0].FileChanges[0].Diff)
		}
	})
}

func TestPatchRejectsUnsupportedSelectorJSONErrors(t *testing.T) {
	path := writeBehavioralPlan(t, t.TempDir())
	patch := []byte("*** Begin Patch\n*** Update Field: implementation[1].file_changes[1].diff\n-old\n+new\n*** End Patch\n")
	var stdout, stderr bytes.Buffer
	withStdin(t, patch, func() {
		if exit := Execute([]string{"patch", "--json-errors", path}, &stdout, &stderr); exit != 1 {
			t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
		}
	})
	code, msg := firstStderrJSON(t, &stderr)
	if code != "VALIDATE_INPUT" {
		t.Fatalf("code=%q want VALIDATE_INPUT", code)
	}
	if !strings.Contains(msg, "unsupported patch selector") {
		t.Fatalf("message %q missing unsupported-selector error", msg)
	}
}

func TestPatchPreservesWrappedFrontmatter(t *testing.T) {
	plan, err := DecodePlan(validPlanJSON())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	frontmatter := "---\ntags:\n  - \"#Ticket\"\ntype: issue\nstatus: open\ntemplate_version: 1\nproject: PDEV-083\ndate_created: 2026-05-12\ntopics: []\n---\n\n"
	path := filepath.Join(t.TempDir(), "plan.md")
	if err := os.WriteFile(path, []byte(frontmatter+rendered), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}
	patch := []byte("*** Begin Patch\n*** Update Field: title\n-T\n+Renamed\n*** End Patch\n")
	var stdout, stderr bytes.Buffer
	withStdin(t, patch, func() {
		if exit := Execute([]string{"patch", path}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
		}
	})
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.HasPrefix(string(raw), frontmatter) {
		t.Fatalf("frontmatter changed:\n%s", string(raw))
	}
	assertParsed(t, path, func(plan Plan) {
		if plan.Title != "Renamed" {
			t.Fatalf("title=%q", plan.Title)
		}
	})
}

func TestPatchRerendersCanonically(t *testing.T) {
	path := writeBehavioralPlan(t, t.TempDir())
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	mutated := strings.Replace(string(raw), "## Verification\n---\n", "## Verification\n---\n\n", 1)
	if err := os.WriteFile(path, []byte(mutated), 0o644); err != nil {
		t.Fatal(err)
	}
	patch := []byte("*** Begin Patch\n*** Update Field: title\n-T\n+Renamed\n*** End Patch\n")
	var stdout, stderr bytes.Buffer
	withStdin(t, patch, func() {
		if exit := Execute([]string{"patch", path}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
		}
	})
	updated, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if strings.Contains(string(updated), "## Verification\n---\n\n") {
		t.Fatalf("expected canonical rerender:\n%s", string(updated))
	}
}

func TestPatchWritesAlternateOutputPath(t *testing.T) {
	dir := t.TempDir()
	sourcePath := writeBehavioralPlan(t, dir)
	outPath := filepath.Join(dir, "out.md")
	patch := []byte("*** Begin Patch\n*** Update Field: title\n-T\n+Renamed\n*** End Patch\n")
	var stdout, stderr bytes.Buffer
	withStdin(t, patch, func() {
		if exit := Execute([]string{"patch", sourcePath, outPath}, &stdout, &stderr); exit != 0 {
			t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
		}
	})
	assertParsed(t, sourcePath, func(plan Plan) {
		if plan.Title != "T" {
			t.Fatalf("source title changed: %q", plan.Title)
		}
	})
	assertParsed(t, outPath, func(plan Plan) {
		if plan.Title != "Renamed" {
			t.Fatalf("out title=%q", plan.Title)
		}
	})
}

func TestBehavioralEditsCoverApprovedGrammar(t *testing.T) {
	dir := t.TempDir()
	planPath := writeBehavioralPlan(t, dir)
	out := dir + "/out.md"

	runPlannerOK(t, []string{"title", "set", planPath, out, "New title"}, nil)
	assertParsed(t, out, func(p Plan) {
		if p.Title != "New title" {
			t.Fatalf("title=%q", p.Title)
		}
	})

	runPlannerOK(t, []string{"dod", "goal", "set", out, out, "--goal", "1", "renamed goal"}, nil)
	assertParsed(t, out, func(p Plan) {
		if p.DefinitionOfDone.Goals[0].Text != "renamed goal" || p.DefinitionOfDone.Goals[0].Status != StatusDone {
			t.Fatalf("goal not updated with status preserved: %#v", p.DefinitionOfDone.Goals[0])
		}
	})

	runPlannerOK(t, []string{"implementation", "step", "file-change", "add", out, out, "--step", "1", "--filename", "f", "--explanation", "second", "--diff-stdin"}, []byte("@@ -1 +1 @@\n-x\n+y"))
	runPlannerOK(t, []string{"implementation", "step", "file-change", "filename", "set", out, out, "--step", "1", "--change", "2", "renamed"}, nil)
	runPlannerOK(t, []string{"implementation", "step", "file-change", "diff", "set", out, out, "--step", "1", "--change", "2", "--stdin"}, []byte("raw diff bytes"))
	assertParsed(t, out, func(p Plan) {
		if got := p.Implementation[0].FileChanges[1].Filename; got != "renamed" {
			t.Fatalf("second filename=%q", got)
		}
		if got := p.Implementation[0].FileChanges[1].Diff; got != "raw diff bytes" {
			t.Fatalf("second diff=%q", got)
		}
	})

	runPlannerOK(t, []string{"verification", "automated", "set", out, out, "--item", "1", "new automated"}, nil)
	assertParsed(t, out, func(p Plan) {
		if p.Verification.Automated[0].Text != "new automated" || p.Verification.Automated[0].Status != StatusDone {
			t.Fatalf("automated not updated with status preserved: %#v", p.Verification.Automated[0])
		}
	})
}

func TestBehavioralRemovalAndUsageFailures(t *testing.T) {
	dir := t.TempDir()
	planPath := writeBehavioralPlan(t, dir)
	out := dir + "/out.md"

	for _, tc := range []struct {
		name string
		args []string
		want string
	}{
		{"goal", []string{"dod", "goal", "remove", planPath, out, "--goal", "1"}, "cannot remove the final definition_of_done goal"},
		{"step", []string{"implementation", "step", "remove", planPath, out, "--step", "1"}, "cannot remove the final implementation step"},
		{"change", []string{"implementation", "step", "file-change", "remove", planPath, out, "--step", "1", "--change", "1"}, "cannot remove the final file change from a step"},
		{"json", []string{"--json-errors", "title", "set", planPath, out, "   "}, "USAGE"},
	} {
		t.Run(tc.name, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			if exit := Execute(tc.args, &stdout, &stderr); exit != 2 {
				t.Fatalf("exit=%d want 2 stderr=%q", exit, stderr.String())
			}
			if !strings.Contains(stderr.String(), tc.want) {
				t.Fatalf("stderr missing %q: %q", tc.want, stderr.String())
			}
		})
	}
}

func TestBehavioralEditRejectsUnknownValueFlag(t *testing.T) {
	dir := t.TempDir()
	planPath := writeBehavioralPlan(t, dir)
	out := dir + "/out.md"

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"title", "set", planPath, out, "New title", "--typo", "x"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d want 2 stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stderr.String(), "unknown flag --typo") {
		t.Fatalf("stderr missing unknown flag: %q", stderr.String())
	}
	if _, err := os.Stat(out); !os.IsNotExist(err) {
		t.Fatalf("output should not be written, stat err = %v", err)
	}
}

func TestStructuredBehavioralEditMalformedMarkdownJSONError(t *testing.T) {
	dir := t.TempDir()
	bad := dir + "/bad.md"
	out := dir + "/out.md"
	if err := os.WriteFile(bad, []byte("not a valid planner plan"), 0o644); err != nil {
		t.Fatal(err)
	}

	var stdout, stderr bytes.Buffer
	exit := Execute([]string{"--json-errors", "dod", "goal", "set", bad, out, "--goal", "1", "renamed"}, &stdout, &stderr)
	if exit != 1 {
		t.Fatalf("exit=%d want 1 stderr=%q", exit, stderr.String())
	}
	code, _ := firstStderrJSON(t, &stderr)
	if code != "DECODE_INPUT" {
		t.Fatalf("code=%q want DECODE_INPUT", code)
	}
	if _, err := os.Stat(out); !os.IsNotExist(err) {
		t.Fatalf("output should not be written, stat err = %v", err)
	}
}

func TestOverviewSetSameFilePreservesFrontmatter(t *testing.T) {
	plan, err := DecodePlan(validPlanJSON())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	frontmatter := "---\ntags:\n  - \"#Ticket\"\ntype: issue\nstatus: open\ntemplate_version: 1\nproject: PDEV-083\ndate_created: 2026-05-12\ntopics: []\n---\n\n"
	path := filepath.Join(t.TempDir(), "plan.md")
	if err := os.WriteFile(path, []byte(frontmatter+rendered), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}

	runPlannerOK(t, []string{"overview", "set", path, path, "Updated overview"}, nil)

	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	if !strings.HasPrefix(string(raw), frontmatter) {
		t.Fatalf("frontmatter changed:\n%s", string(raw))
	}
	assertParsed(t, path, func(p Plan) {
		if p.Overview != "Updated overview" {
			t.Fatalf("overview=%q", p.Overview)
		}
	})
}

func TestImplementationStepSummarySetSameFilePreservesWrappedFrontmatter(t *testing.T) {
	plan, err := DecodePlan(validPlanJSON())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	frontmatter := "---\ntags:\n  - \"#Ticket\"\ntype: issue\nstatus: open\ntemplate_version: 1\nproject: PDEV-083\ndate_created: 2026-05-12\ntopics: []\n---\n\n"
	path := filepath.Join(t.TempDir(), "plan.md")
	if err := os.WriteFile(path, []byte(frontmatter+rendered), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}

	runPlannerOK(t, []string{"implementation", "step", "summary", "set", path, path, "--step", "1", "Updated summary"}, nil)

	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	if !strings.HasPrefix(string(raw), frontmatter) {
		t.Fatalf("frontmatter changed:\n%s", string(raw))
	}
	assertParsed(t, path, func(p Plan) {
		if p.Implementation[0].Summary != "Updated summary" {
			t.Fatalf("summary=%q", p.Implementation[0].Summary)
		}
	})
}

// withStdin routes data through os.Stdin for the duration of fn via a real
// os.Pipe (no mock). Tests exercise the production Execute path end-to-end.
func withStdin(t *testing.T, data []byte, fn func()) {
	t.Helper()
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatalf("pipe: %v", err)
	}
	original := os.Stdin
	os.Stdin = r
	defer func() { os.Stdin = original }()
	go func() { defer w.Close(); _, _ = w.Write(data) }()
	fn()
}

func writeNewScaffold(t *testing.T, dir string) string {
	t.Helper()
	path := dir + "/plan.md"
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"new", path}, &stdout, &stderr); exit != 0 {
		t.Fatalf("Execute(new %s) exit=%d stderr=%q", path, exit, stderr.String())
	}
	if _, err := os.Stat(path); err != nil {
		t.Fatalf("expected scaffold at %s: %v", path, err)
	}
	return path
}

func writeBehavioralPlan(t *testing.T, dir string) string {
	t.Helper()
	path := dir + "/plan.md"
	plan, err := DecodePlan(mustJSON(Plan{
		Title:    "T",
		Overview: "O",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "N",
			Goals:        []ChecklistItem{{Text: "g", Status: StatusDone}},
			CurrentState: "C",
			ModuleShape:  "M",
		},
		Implementation: []Step{{
			Title:   "T",
			Summary: "S",
			FileChanges: []FileChange{{
				Filename:    "f",
				Explanation: "e",
				Diff:        "@@ -1 +1 @@\n-a\n+b",
			}},
		}},
		Verification: &Verification{
			Summary:   "",
			Automated: []ChecklistItem{{Text: "A", Status: StatusDone}},
			Manual:    []ChecklistItem{{Text: "M"}},
		},
	}))
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	if err := CreatePlanFromStruct(plan, path); err != nil {
		t.Fatalf("CreatePlanFromStruct: %v", err)
	}
	return path
}

func runPlannerOK(t *testing.T, args []string, stdin []byte) {
	t.Helper()
	run := func() {
		var stdout, stderr bytes.Buffer
		if exit := Execute(args, &stdout, &stderr); exit != 0 {
			t.Fatalf("Execute(%v) exit=%d stderr=%q stdout=%q", args, exit, stderr.String(), stdout.String())
		}
	}
	if stdin != nil {
		withStdin(t, stdin, run)
		return
	}
	run()
}

func assertParsed(t *testing.T, path string, check func(Plan)) {
	t.Helper()
	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("ReadFile: %v", err)
	}
	parsed, err := ParseMarkdown(string(raw))
	if err != nil {
		t.Fatalf("ParseMarkdown: %v\n%s", err, string(raw))
	}
	check(parsed.Plan)
}

func validPlanJSON() []byte {
	return mustJSON(Plan{
		Title:    "T",
		Overview: "O",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "N",
			Goals:        []ChecklistItem{{Text: "g"}},
			CurrentState: "C",
			ModuleShape:  "M",
		},
		Implementation: []Step{{
			Title:   "T",
			Summary: "S",
			FileChanges: []FileChange{{
				Filename:    "f",
				Explanation: "e",
				Diff:        "@@ -1 +1 @@\n-a\n+b",
			}},
		}},
		Verification: &Verification{
			Summary:   "",
			Automated: []ChecklistItem{{Text: "A"}},
			Manual:    []ChecklistItem{{Text: "M"}},
		},
	})
}

func validPlanJSONMap(t *testing.T, kv map[string]any) []byte {
	t.Helper()
	var doc map[string]any
	if err := json.Unmarshal(validPlanJSON(), &doc); err != nil {
		t.Fatalf("json.Unmarshal: %v", err)
	}
	for key, value := range kv {
		doc[key] = value
	}
	return mustJSON(doc)
}

func mustJSON(v any) []byte {
	raw, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	return raw
}

func TestInspectOutputIsValidPlan(t *testing.T) {
	dir := t.TempDir()
	src := dir + "/plan.md"
	plan, err := DecodePlan(validPlanJSON())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	if err := CreatePlanFromStruct(plan, src); err != nil {
		t.Fatalf("CreatePlanFromStruct: %v", err)
	}

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"inspect", src}, &stdout, &stderr); exit != 0 {
		t.Fatalf("inspect exit %d stderr %q", exit, stderr.String())
	}

	inspectedPlan, err := DecodePlan(stdout.Bytes())
	if err != nil {
		t.Fatalf("inspect output not valid plan JSON: %v", err)
	}
	if err := ValidatePlan(inspectedPlan); err != nil {
		t.Fatalf("inspect output does not validate: %v", err)
	}
	if inspectedPlan.Title == "" || len(inspectedPlan.Implementation) == 0 || inspectedPlan.Verification == nil {
		t.Fatalf("inspect output missing plan content: %#v", inspectedPlan)
	}
}

func TestInspectOutputOmitsFrontmatterFields(t *testing.T) {
	dir := t.TempDir()
	src := dir + "/plan.md"
	plan, err := DecodePlan(validPlanJSON())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatalf("RenderPlan: %v", err)
	}
	if err := os.WriteFile(src, []byte("---\ntags:\n  - \"#Ticket\"\ntype: issue\nstatus: open\ntemplate_version: 1\nproject: PDEV-083\ndate_created: 2026-05-12\ntopics: []\n---\n\n"+rendered), 0o644); err != nil {
		t.Fatal(err)
	}

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"inspect", src}, &stdout, &stderr); exit != 0 {
		t.Fatalf("inspect exit %d stderr %q", exit, stderr.String())
	}
	outPlan, err := DecodePlan(stdout.Bytes())
	if err != nil {
		t.Fatalf("DecodePlan: %v", err)
	}
	if err := ValidatePlan(outPlan); err != nil {
		t.Fatalf("ValidatePlan: %v", err)
	}
	for _, want := range []string{"\"tags\"", "\"type\"", "\"template_version\"", "\"topics\"", "\"project\"", "\"date_created\""} {
		if strings.Contains(stdout.String(), want) {
			t.Fatalf("inspect output unexpectedly contains %q: %q", want, stdout.String())
		}
	}
}

func TestReplaceCommandRemoved(t *testing.T) {
	var stdout bytes.Buffer
	var stderr bytes.Buffer

	if exitCode := Execute([]string{"replace"}, &stdout, &stderr); exitCode != 2 {
		t.Fatalf("Execute(replace) exit code = %d, want 2", exitCode)
	}
	if !strings.Contains(stderr.String(), "unknown command: replace") {
		t.Fatalf("replace stderr missing unknown-command message: %q", stderr.String())
	}
}

func TestJSONErrorsCoversUnknownCommand(t *testing.T) {
	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"--json-errors", "bogus"}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit %d want 2; stderr %q", exit, stderr.String())
	}
	code, _ := firstStderrJSON(t, &stderr)
	if code != "USAGE" {
		t.Fatalf("code=%q want USAGE", code)
	}
}

// firstStderrJSON unmarshals the first non-empty stderr line as the planner
// error envelope. Tests use it to assert the --json-errors contract: every
// failure path emits one parseable JSON object with a stable code.
func firstStderrJSON(t *testing.T, stderr *bytes.Buffer) (code, message string) {
	t.Helper()
	line := bytes.TrimSpace(stderr.Bytes())
	if len(line) == 0 {
		t.Fatal("stderr is empty")
	}
	if nl := bytes.IndexByte(line, '\n'); nl >= 0 {
		line = line[:nl]
	}
	var got struct {
		Code    string `json:"code"`
		Message string `json:"message"`
	}
	if err := json.Unmarshal(line, &got); err != nil {
		t.Fatalf("first stderr line is not JSON: %v; raw=%q", err, stderr.String())
	}
	return got.Code, got.Message
}
