package internal

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// Wrapped vault docs must still decode through check, but check now also needs
// a base commit and applicable diffs. A valid wrapper reaches the base commit
// rule; a broken wrapper still fails at decode before any Git work.
func TestWrappedCheck(t *testing.T) {
	for _, tc := range []struct {
		name     string
		fixture  string
		wantCode string
		wantHint string
	}{
		{name: "empty_topics", fixture: "wrapped_issue_empty_topics.md",
			wantCode: codeBaseCommitRequired, wantHint: "Base commit"},
		{name: "extra_tag", fixture: "wrapped_issue_extra_tag.md",
			wantCode: codeBaseCommitRequired, wantHint: "Base commit"},
		{name: "topic_list", fixture: "wrapped_issue_topics.md",
			wantCode: codeBaseCommitRequired, wantHint: "Base commit"},
		{name: "bad_tag", fixture: "wrapped_issue_bad_tag.md",
			wantCode: "DECODE_INPUT", wantHint: "supported vault issue frontmatter block"},
		{name: "missing_ticket_tag", fixture: "wrapped_issue_missing_ticket_tag.md",
			wantCode: "DECODE_INPUT", wantHint: "supported vault issue frontmatter block"},
	} {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			path := copyFixture(t, tc.fixture)
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			if exit := Execute([]string{"check", "--json-errors", path}, &stdout, &stderr); exit != 1 {
				t.Fatalf("exit=%d want 1 stderr=%q", exit, stderr.String())
			}
			assertPlannerJSONError(t, &stderr, tc.wantCode, tc.wantHint)
		})
	}
}

func TestWrappedInspect(t *testing.T) {
	for _, tc := range []struct {
		name     string
		fixture  string
		wantExit int
		wantCode string
	}{
		{name: "extra_tag", fixture: "wrapped_issue_extra_tag.md", wantExit: 0},
		{name: "topic_list", fixture: "wrapped_issue_topics.md", wantExit: 0},
		{name: "bad_tag", fixture: "wrapped_issue_bad_tag.md", wantExit: 1, wantCode: "DECODE_INPUT"},
		{name: "missing_ticket_tag", fixture: "wrapped_issue_missing_ticket_tag.md", wantExit: 1, wantCode: "DECODE_INPUT"},
	} {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			path := copyFixture(t, tc.fixture)
			var stdout bytes.Buffer
			var stderr bytes.Buffer
			args := []string{"inspect", path}
			if tc.wantCode != "" {
				args = []string{"inspect", "--json-errors", path}
			}
			if exit := Execute(args, &stdout, &stderr); exit != tc.wantExit {
				t.Fatalf("exit=%d want %d stderr=%q", exit, tc.wantExit, stderr.String())
			}
			if tc.wantCode != "" {
				assertPlannerJSONError(t, &stderr, tc.wantCode, "supported vault issue frontmatter block")
				return
			}
			if strings.Contains(stdout.String(), `"tags"`) || strings.Contains(stdout.String(), `"topics"`) {
				t.Fatalf("inspect leaked frontmatter: %q", stdout.String())
			}
			var view InspectPlan
			if err := json.Unmarshal(stdout.Bytes(), &view); err != nil {
				t.Fatalf("inspect output: %v", err)
			}
			if view.Title != "Sample Wrapped Plan" {
				t.Fatalf("title=%q", view.Title)
			}
		})
	}
}

func TestWrappedDecodeFailures(t *testing.T) {
	fixtures := []string{
		"wrapped_issue_bad_tag.md",
		"wrapped_issue_empty_topics_block.md",
		"wrapped_issue_missing_ticket_tag.md",
		"wrapped_issue_reordered_fields.md",
		"wrapped_issue_duplicate_status.md",
	}
	commands := []struct {
		name string
		run  func(*testing.T, string) (int, string)
	}{
		{
			name: "check",
			run: func(t *testing.T, path string) (int, string) {
				t.Helper()
				var stdout bytes.Buffer
				var stderr bytes.Buffer
				exit := Execute([]string{"check", "--json-errors", path}, &stdout, &stderr)
				return exit, stderr.String()
			},
		},
		{
			name: "inspect",
			run: func(t *testing.T, path string) (int, string) {
				t.Helper()
				var stdout bytes.Buffer
				var stderr bytes.Buffer
				exit := Execute([]string{"inspect", "--json-errors", path}, &stdout, &stderr)
				return exit, stderr.String()
			},
		},
	}

	for _, fixture := range fixtures {
		fixture := fixture
		for _, command := range commands {
			command := command
			t.Run(fixture+"/"+command.name, func(t *testing.T) {
				path := copyFixture(t, fixture)
				exit, stderrText := command.run(t, path)
				if exit != 1 {
					t.Fatalf("exit=%d want 1 stderr=%q", exit, stderrText)
				}
				stderr := bytes.NewBufferString(stderrText)
				assertPlannerJSONError(t, stderr, "DECODE_INPUT", "supported vault issue frontmatter block")
			})
		}
	}
}

// Help must advertise only the commands that still exist and must not revive
// removed grammar. An AI discovering the surface from `planner help` alone
// would otherwise try deleted structured patch or behavioral commands.
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

	// Negative anchors: deleted commands and removed grammar must not reappear.
	for _, banned := range []string{
		"show-schema",
		"planner generate",
		"planner replace",
		"--write",
		"planner dod",
		"planner implementation",
		"planner verification",
		"*** Update Field",
		"*** Update Diff",
		"*** Begin Patch",
		"behavioral edit flags",
	} {
		if strings.Contains(help, banned) {
			t.Fatalf("buildHelpText() still mentions removed token %q", banned)
		}
	}
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
		{name: "non_md", args: []string{"--json-errors", "new", badOut}, want: "planner new requires an output path ending in .md: usage: planner new <output.md> [--issue --project NAME] [--diff] [--dry-run] [--json-errors]"},
		{name: "missing_output", args: []string{"new", "--json-errors"}, want: "usage: planner new <output.md> [--issue --project NAME] [--diff] [--dry-run] [--json-errors]"},
		{name: "extra_output", args: []string{"new", badOut, "extra", "--json-errors"}, want: "usage: planner new <output.md> [--issue --project NAME] [--diff] [--dry-run] [--json-errors]"},
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

// inspect must accept a fresh scaffold even though check rejects it, so the
// inspect and patch authoring flow works before any diff exists.
func TestNewScaffoldInspectPasses(t *testing.T) {
	path := writeNewScaffold(t, t.TempDir())

	var inspectStdout bytes.Buffer
	var inspectStderr bytes.Buffer
	if exit := Execute([]string{"inspect", path}, &inspectStdout, &inspectStderr); exit != 0 {
		t.Fatalf("Execute(inspect) exit = %d, stderr = %q", exit, inspectStderr.String())
	}
	var inspected InspectPlan
	if err := json.Unmarshal(inspectStdout.Bytes(), &inspected); err != nil {
		t.Fatalf("inspect output is not valid inspect JSON: %v", err)
	}
	if inspected.Title == "" || len(inspected.Implementation) == 0 || inspected.Verification == nil {
		t.Fatalf("inspect output missing inspect content: %#v", inspected)
	}
	if inspected.Implementation[0].FileChanges[0].Selector != "implementation[1].file_changes[1]" {
		t.Fatalf("selector=%q", inspected.Implementation[0].FileChanges[0].Selector)
	}
	if !strings.HasPrefix(inspected.Implementation[0].FileChanges[0].UpdateDiffExpect, "sha256:") {
		t.Fatalf("token=%q", inspected.Implementation[0].FileChanges[0].UpdateDiffExpect)
	}
}

// A fresh scaffold has a placeholder diff and no recorded base commit, so check
// must reject it. The old plain check printed OK here, which hid unrevised plans.
func TestNewScaffoldCheckRequiresBaseCommit(t *testing.T) {
	path := writeNewScaffold(t, t.TempDir())

	var stdout bytes.Buffer
	var stderr bytes.Buffer
	if exit := Execute([]string{"check", "--json-errors", path}, &stdout, &stderr); exit != 1 {
		t.Fatalf("Execute(check) exit = %d, stderr = %q", exit, stderr.String())
	}
	assertPlannerJSONError(t, &stderr, codeBaseCommitRequired, "Base commit")
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

func TestNewPreservesExistingFile(t *testing.T) {
	for _, tc := range []struct {
		name string
		args []string
	}{
		{name: "write"},
		{name: "dry_run", args: []string{"--dry-run"}},
		{name: "diff_dry_run", args: []string{"--diff", "--dry-run"}},
	} {
		tc := tc
		t.Run(tc.name, func(t *testing.T) {
			path := filepath.Join(t.TempDir(), "plan.md")
			if err := os.WriteFile(path, []byte("sentinel\n"), 0o644); err != nil {
				t.Fatalf("WriteFile: %v", err)
			}

			var stdout, stderr bytes.Buffer
			if exit := Execute(append([]string{"new", path}, tc.args...), &stdout, &stderr); exit != 1 {
				t.Fatalf("exit=%d stderr=%q stdout=%q", exit, stderr.String(), stdout.String())
			}
			got, err := os.ReadFile(path)
			if err != nil {
				t.Fatalf("ReadFile: %v", err)
			}
			if string(got) != "sentinel\n" {
				t.Fatalf("existing file changed: %q", got)
			}
		})
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

// check applies the plan's diffs at the recorded base commit and reports what
// it applied, so a plan whose diffs do not apply cannot pass review.
func TestCheckAppliesPlanDiffs(t *testing.T) {
	repo, baseCommit, name := checkPlanFixture(t)

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", name, "--repo", repo, "--json-errors"}, &stdout, &stderr); exit != 0 {
		t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
	}
	result := decodeGuardedResult[guardedCheckResult](t, stdout.String())
	if !result.StructureValid || !result.ApplicabilityChecked ||
		result.ChangesApplied != 1 || result.ChangesReplayed != 1 ||
		result.ChangesApplied != result.ChangesReplayed {
		t.Fatalf("unexpected result: %+v", result)
	}
	if result.BaseCommit != baseCommit {
		t.Fatalf("base commit=%q want %q", result.BaseCommit, baseCommit)
	}
	if result.BehaviorChecked {
		t.Fatal("check must not claim to check behavior")
	}
}

// Supported vault issue frontmatter must be stripped before the plan is
// checked, so a wrapped plan with an applicable diff still passes.
func TestCheckAcceptsIssueFrontmatter(t *testing.T) {
	repo, baseCommit, name := checkPlanFixture(t)
	frontmatter := "---\ntags:\n  - \"#Ticket\"\ntype: issue\nstatus: open\ntemplate_version: 1\nproject: PDEV-201\ndate_created: 2026-10-02\ntopics: []\n---\n\n"
	raw, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(name, []byte(frontmatter+string(raw)), 0o644); err != nil {
		t.Fatal(err)
	}

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"check", name, "--repo", repo, "--json-errors"}, &stdout, &stderr); exit != 0 {
		t.Fatalf("exit=%d stderr=%q", exit, stderr.String())
	}
	result := decodeGuardedResult[guardedCheckResult](t, stdout.String())
	if result.BaseCommit != baseCommit {
		t.Fatalf("base commit=%q want %q", result.BaseCommit, baseCommit)
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

// planner patch always routes to the guarded handler. A missing --target must
// fail as a usage error before touching the plan, so a stray patch invocation
// cannot rewrite an approved plan.
func TestPatchWithoutTargetIsUsageError(t *testing.T) {
	path := writeNewScaffold(t, t.TempDir())
	before, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"patch", path}, &stdout, &stderr); exit != 2 {
		t.Fatalf("exit=%d want 2; stderr=%q", exit, stderr.String())
	}
	if !strings.Contains(stderr.String(), "--target is required") {
		t.Fatalf("stderr missing --target usage: %q", stderr.String())
	}
	after, err := os.ReadFile(path)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatal("failed patch changed the plan")
	}
}

// The dod, implementation, and verification setter commands were removed. They
// must be unknown commands so automation cannot keep depending on a deleted
// write path.
func TestBehavioralCommandsRemoved(t *testing.T) {
	for _, command := range []string{"dod", "implementation", "verification"} {
		command := command
		t.Run(command, func(t *testing.T) {
			var stdout, stderr bytes.Buffer
			if exit := Execute([]string{command}, &stdout, &stderr); exit != 2 {
				t.Fatalf("exit=%d want 2; stderr=%q", exit, stderr.String())
			}
			if !strings.Contains(stderr.String(), "unknown command: "+command) {
				t.Fatalf("stderr %q missing unknown-command error for %s", stderr.String(), command)
			}
		})
	}
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

// chdir moves the test process into dir and restores the previous directory when
// the test ends. Tests run sequentially, so no other test observes the change.
func chdir(t *testing.T, dir string) {
	t.Helper()
	old, err := os.Getwd()
	if err != nil {
		t.Fatal(err)
	}
	if err := os.Chdir(dir); err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := os.Chdir(old); err != nil {
			t.Fatalf("restore working directory: %v", err)
		}
	})
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

// twoStepPlan is a two-step implementation fixture shared by selector tests.
func twoStepPlan() Plan {
	return Plan{
		Title:    "Plan",
		Overview: "Overview",
		DefinitionOfDone: DefinitionOfDone{
			Narrative:    "Narrative",
			Goals:        []ChecklistItem{{Text: "Goal"}},
			CurrentState: "Current",
			ModuleShape:  "Shape",
		},
		Implementation: []Step{
			{
				Title:   "First",
				Summary: "Summary1",
				FileChanges: []FileChange{{
					Filename:    "a.go",
					Explanation: "why",
					Diff:        "@@ -1 +1 @@\n-old\n+new",
				}},
			},
			{
				Title:   "Second",
				Summary: "Summary2",
				FileChanges: []FileChange{{
					Filename:    "b.go",
					Explanation: "why",
					Diff:        "@@ -1 +1 @@\n-old\n+new",
				}},
			},
		},
		Verification: &Verification{
			Summary:   "Summary",
			Automated: []ChecklistItem{{Text: "go test ./..."}},
			Manual:    []ChecklistItem{{Text: "smoke"}},
		},
	}
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

func mustJSON(v any) []byte {
	raw, err := json.Marshal(v)
	if err != nil {
		panic(err)
	}
	return raw
}

// inspect must accept a rendered plan file and emit valid inspect JSON with a
// stable selector and edit token, without depending on a removed writer.
func TestInspectOutputIsValidPlan(t *testing.T) {
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
	if err := os.WriteFile(src, []byte(rendered), 0o644); err != nil {
		t.Fatalf("WriteFile: %v", err)
	}

	var stdout, stderr bytes.Buffer
	if exit := Execute([]string{"inspect", src}, &stdout, &stderr); exit != 0 {
		t.Fatalf("inspect exit %d stderr %q", exit, stderr.String())
	}

	var inspected InspectPlan
	if err := json.Unmarshal(stdout.Bytes(), &inspected); err != nil {
		t.Fatalf("inspect output not valid inspect JSON: %v", err)
	}
	if inspected.Title == "" || len(inspected.Implementation) == 0 || inspected.Verification == nil {
		t.Fatalf("inspect output missing inspect content: %#v", inspected)
	}
	if inspected.Implementation[0].FileChanges[0].Selector != "implementation[1].file_changes[1]" {
		t.Fatalf("selector=%q", inspected.Implementation[0].FileChanges[0].Selector)
	}
	if !strings.HasPrefix(inspected.Implementation[0].FileChanges[0].UpdateDiffExpect, "sha256:") {
		t.Fatalf("token=%q", inspected.Implementation[0].FileChanges[0].UpdateDiffExpect)
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
	var inspected InspectPlan
	if err := json.Unmarshal(stdout.Bytes(), &inspected); err != nil {
		t.Fatalf("inspect output: %v", err)
	}
	if inspected.Verification == nil || inspected.Implementation[0].FileChanges[0].UpdateDiffExpect == "" {
		t.Fatalf("inspect output missing diff metadata: %#v", inspected)
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

func assertPlannerJSONError(t *testing.T, stderr *bytes.Buffer, wantCode, wantHint string) {
	t.Helper()
	line := bytes.TrimSpace(stderr.Bytes())
	if len(line) == 0 {
		t.Fatal("stderr is empty")
	}
	if nl := bytes.IndexByte(line, '\n'); nl >= 0 {
		line = line[:nl]
	}
	var got struct {
		Code         string `json:"code"`
		Message      string `json:"message"`
		RecoveryHint string `json:"recovery_hint"`
	}
	if err := json.Unmarshal(line, &got); err != nil {
		t.Fatalf("first stderr line is not JSON: %v; raw=%q", err, stderr.String())
	}
	if got.Code != wantCode {
		t.Fatalf("code=%q want %q", got.Code, wantCode)
	}
	if !strings.Contains(got.RecoveryHint, wantHint) {
		t.Fatalf("recovery hint %q missing %q", got.RecoveryHint, wantHint)
	}
}

func copyFixture(t *testing.T, name string) string {
	t.Helper()
	raw, err := os.ReadFile(filepath.Join("testdata", name))
	if err != nil {
		t.Fatalf("ReadFile(fixture): %v", err)
	}
	path := filepath.Join(t.TempDir(), name)
	if err := os.WriteFile(path, raw, 0o644); err != nil {
		t.Fatalf("WriteFile(fixture): %v", err)
	}
	return path
}
