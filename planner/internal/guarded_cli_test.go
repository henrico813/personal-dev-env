package internal

import (
	"bytes"
	"encoding/json"
	"errors"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"planner/internal/planpatch"
)

func revisionGit(t *testing.T, repo string, args ...string) string {
	t.Helper()
	cmd := exec.Command("git", append([]string{"-C", repo}, args...)...)
	for _, item := range os.Environ() {
		if !strings.HasPrefix(item, "GIT_") {
			cmd.Env = append(cmd.Env, item)
		}
	}
	cmd.Env = append(cmd.Env, "GIT_CONFIG_NOSYSTEM=1",
		"GIT_CONFIG_GLOBAL="+os.DevNull)
	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git: %v: %s", err, out)
	}
	return strings.TrimSpace(string(out))
}

func revisionRepoContent(t *testing.T, content string, perm os.FileMode) (repo, baseCommit string) {
	t.Helper()
	repo = t.TempDir()
	revisionGit(t, repo, "init", "--quiet", "--template=")
	if err := os.WriteFile(filepath.Join(repo, "foo.txt"), []byte(content), perm); err != nil {
		t.Fatal(err)
	}
	revisionGit(t, repo, "add", "foo.txt")
	revisionGit(t, repo, "-c", "user.name=Test",
		"-c", "user.email=test@example.invalid",
		"-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "base commit")
	return repo, revisionGit(t, repo, "rev-parse", "HEAD")
}

func revisionRepo(t *testing.T, perm os.FileMode) (repo, baseCommit string) {
	t.Helper()
	return revisionRepoContent(t, "A\n", perm)
}

// fooDiff generates a plan-embedded diff through Git so the fixture never
// hand-maintains hunk headers, matching how the guarded patch path works.
func fooDiff(t *testing.T, before, after, mode string) string {
	t.Helper()
	raw, err := planpatch.Generate("foo.txt",
		&planpatch.File{Data: []byte(before), Mode: mode},
		&planpatch.File{Data: []byte(after), Mode: mode})
	if err != nil {
		t.Fatal(err)
	}
	return strings.TrimSuffix(string(raw), "\n")
}

// writeRenderedPlan renders plan to a fresh temp Markdown file.
func writeRenderedPlan(t *testing.T, plan Plan) string {
	t.Helper()
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatal(err)
	}
	name := filepath.Join(t.TempDir(), "plan.md")
	if err := os.WriteFile(name, []byte(rendered), 0644); err != nil {
		t.Fatal(err)
	}
	return name
}

// checkPlanFixture renders a plan whose single foo.txt diff applies and whose
// Current State records the base commit as its first line, matching
// create-plan output.
func checkPlanFixture(t *testing.T) (repo, baseCommit, name string) {
	t.Helper()
	repo, baseCommit = revisionRepo(t, 0644)
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "foo.txt"
	plan.Implementation[0].FileChanges[0].Diff = fooDiff(t, "A\n", "B\n", "100644")
	plan.DefinitionOfDone.CurrentState = "Base commit: " + baseCommit
	return repo, baseCommit, writeRenderedPlan(t, plan)
}

// writeRevisionPlan renders the plan and inserts a human review note. A targeted
// edit must keep that note; rendering the whole document again would drop text
// the parser does not model.
func writeRevisionPlan(t *testing.T, plan Plan) string {
	t.Helper()
	rendered, err := RenderPlan(plan)
	if err != nil {
		t.Fatal(err)
	}
	rendered = strings.Replace(rendered, "### Current State",
		"Human review note: keep this.\n\n### Current State", 1)
	name := filepath.Join(t.TempDir(), "plan.md")
	if err := os.WriteFile(name, []byte(rendered), 0644); err != nil {
		t.Fatal(err)
	}
	return name
}

func revisionFixture(t *testing.T, twoSteps bool) (repo, baseCommit, name string) {
	t.Helper()
	repo, baseCommit = revisionRepo(t, 0644)
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "foo.txt"
	plan.Implementation[0].FileChanges[0].Diff = fooDiff(t, "A\n", "B\n", "100644")
	if twoSteps {
		plan.Implementation = append(plan.Implementation, Step{
			Title:   "Second edit",
			Summary: "Extend the earlier result.",
			FileChanges: []FileChange{{
				Filename:    "foo.txt",
				Explanation: "Complete the change.",
				Diff:        fooDiff(t, "B\n", "C\n", "100644"),
			}},
		})
	}
	return repo, baseCommit, writeRevisionPlan(t, plan)
}

// revisionExecutableFixture commits foo.txt with the executable bit so the
// patch path can prove it retains mode 100755 instead of resetting to 100644.
func revisionExecutableFixture(t *testing.T) (repo, baseCommit, name string) {
	t.Helper()
	repo, baseCommit = revisionRepo(t, 0755)
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "foo.txt"
	plan.Implementation[0].FileChanges[0].Diff = fooDiff(t, "A\n", "B\n", "100755")
	return repo, baseCommit, writeRevisionPlan(t, plan)
}

// revisionBrokenSecondFixture leaves step 2 as an unusable PLACEHOLDER. A new
// plan can have a later change that is not written yet while an earlier step is
// revised, so patch must not depend on every later change applying.
func revisionBrokenSecondFixture(t *testing.T) (repo, baseCommit, name string) {
	t.Helper()
	repo, baseCommit = revisionRepo(t, 0644)
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "foo.txt"
	plan.Implementation[0].FileChanges[0].Diff = fooDiff(t, "A\n", "B\n", "100644")
	plan.Implementation = append(plan.Implementation, Step{
		Title:   "Second edit",
		Summary: "Extend the earlier result.",
		FileChanges: []FileChange{{
			Filename:    "foo.txt",
			Explanation: "Complete the change.",
			Diff:        "PLACEHOLDER",
		}},
	})
	return repo, baseCommit, writeRevisionPlan(t, plan)
}

func revisionExecute(args ...string) (int, string, string) {
	var stdout, stderr bytes.Buffer
	code := Execute(append(args, "--json-errors"), &stdout, &stderr)
	return code, stdout.String(), stderr.String()
}

func decodeGuardedResult[T any](t *testing.T, raw string) T {
	t.Helper()
	var value T
	if err := json.Unmarshal([]byte(raw), &value); err != nil {
		t.Fatalf("decode guarded JSON: %v: %q", err, raw)
	}
	return value
}

func revisionInspect(t *testing.T, name, repo, baseCommit, target string,
	extras ...string) guardedInspectResult {
	t.Helper()
	args := []string{"inspect", name, "--target", target,
		"--repo", repo, "--base-commit", baseCommit}
	code, out, diagnostic := revisionExecute(append(args, extras...)...)
	if code != 0 {
		t.Fatalf("inspect %s: exit %d: %s", target, code, diagnostic)
	}
	return decodeGuardedResult[guardedInspectResult](t, out)
}

func revisionPatch(t *testing.T, name, target, expect, repo, baseCommit string,
	extras ...string) guardedPatchResult {
	t.Helper()
	code, out, diagnostic := revisionExecute(patchArgs(name, target, expect,
		repo, baseCommit, extras...)...)
	if code != 0 {
		t.Fatalf("patch %s: exit %d: %s", target, code, diagnostic)
	}
	return decodeGuardedResult[guardedPatchResult](t, out)
}

func patchArgs(name, target, expect, repo, baseCommit string, extra ...string) []string {
	args := []string{"patch", name, "--target", target, "--expect", expect,
		"--repo", repo, "--base-commit", baseCommit}
	return append(args, extra...)
}

type guardedErrorPayload struct {
	Code         string `json:"code"`
	Message      string `json:"message"`
	RecoveryHint string `json:"recovery_hint"`
}

func requireGuardedError(t *testing.T, diagnostic, wantCode string) guardedErrorPayload {
	t.Helper()
	var payload guardedErrorPayload
	if err := json.Unmarshal([]byte(diagnostic), &payload); err != nil {
		t.Fatalf("stderr is not guarded JSON: %v: %q", err, diagnostic)
	}
	if payload.Code != wantCode {
		t.Fatalf("code = %q, want %q (message %q)", payload.Code, wantCode, payload.Message)
	}
	return payload
}

// Inspect with --code-out writes ordinary source from the base commit (the
// full commit ID the plan is measured against). A model edits a file instead of
// a JSON-escaped code string, and the user's checkout is left untouched.
func TestInspectExportsProposedSource(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "proposed.txt")
	view := revisionInspect(t, name, repo, baseCommit, target, "--code-out", scratch)
	code, err := os.ReadFile(scratch)
	if err != nil {
		t.Fatal(err)
	}
	if string(code) != "B\n" {
		t.Fatalf("wrong proposed state: %q", code)
	}
	if view.Diff != "" {
		t.Fatalf("code export unnecessarily repeats full diff: %q", view.Diff)
	}
	if view.CodeExists == nil || !*view.CodeExists {
		t.Fatalf("code_exists = %v, want true", view.CodeExists)
	}
	if view.CodeState != "after_selected_change" {
		t.Fatalf("code_state = %q", view.CodeState)
	}
	if view.Mode != "100644" {
		t.Fatalf("mode = %q, want 100644", view.Mode)
	}
	source, err := os.ReadFile(filepath.Join(repo, "foo.txt"))
	if err != nil {
		t.Fatal(err)
	}
	if string(source) != "A\n" {
		t.Fatal("inspection implemented source changes")
	}
}

// Patching ordinary source back regenerates a Git diff, so a model never
// hand-counts hunk line numbers. The write changes only the selected diff, so
// the human review note next to it stays in the file.
func TestPatchWritesPlanFromScratch(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "proposed.txt")
	view := revisionInspect(t, name, repo, baseCommit, target, "--code-out", scratch)
	if err := os.WriteFile(scratch, []byte("C\n"), 0600); err != nil {
		t.Fatal(err)
	}
	result := revisionPatch(t, name, target, view.EditExpect, repo, baseCommit,
		"--after-file", scratch)
	if !result.Written || !result.PrefixReplayed || result.DownstreamChecked {
		t.Fatalf("unexpected result: %+v", result)
	}
	if result.PlanSHA256 == "" || result.BaseCommit != baseCommit {
		t.Fatalf("result missing hash or base commit: %+v", result)
	}
	changed, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(changed, []byte("Human review note: keep this.")) ||
		!bytes.Contains(changed, []byte("+C")) {
		t.Fatalf("missing edit or review note: %s", changed)
	}
	code, _, diagnostic := revisionExecute("check", name, "--repo", repo, "--base-commit", baseCommit)
	if code != 0 {
		t.Fatalf("final gate: %s", diagnostic)
	}
}

// A stale edit_expect token (a hash of the plan, the selected change, and the
// base commit) must fail before any write, and a malformed replacement diff
// must report PATCH_INVALID. After either failure the plan file must be
// unchanged, so a rejected edit cannot leave a half-written plan.
func TestStaleTokenAndBadDiffRejected(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	view := revisionInspect(t, name, repo, baseCommit, target)
	raw, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	changed := append(append([]byte(nil), raw...), '\n')
	if err := os.WriteFile(name, changed, 0644); err != nil {
		t.Fatal(err)
	}
	diff := filepath.Join(t.TempDir(), "bad.diff")
	if err := os.WriteFile(diff, []byte("this is not a patch"), 0600); err != nil {
		t.Fatal(err)
	}
	code, _, diagnostic := revisionExecute(patchArgs(name, target, view.EditExpect,
		repo, baseCommit, "--diff-file", diff)...)
	if code == 0 {
		t.Fatal("stale token was accepted")
	}
	requireGuardedError(t, diagnostic, planpatch.CodePlanStale)
	view = revisionInspect(t, name, repo, baseCommit, target)
	code, _, diagnostic = revisionExecute(patchArgs(name, target, view.EditExpect,
		repo, baseCommit, "--diff-file", diff)...)
	if code == 0 {
		t.Fatal("bad diff was accepted")
	}
	requireGuardedError(t, diagnostic, planpatch.CodePatchInvalid)
	actual, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(actual, changed) {
		t.Fatal("failed patch modified plan")
	}
}

// A scratch export is a NEW file. Reusing the path must fail with WRITE_OUTPUT
// and leave the earlier export intact, so an inspection cannot overwrite reviewed
// source or the plan itself.
func TestInspectScratchReservation(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "proposed.txt")
	view := revisionInspect(t, name, repo, baseCommit, target, "--code-out", scratch)
	if view.CodeOut != scratch {
		t.Fatalf("code_out = %q, want %q", view.CodeOut, scratch)
	}
	code, _, diagnostic := revisionExecute("inspect", name, "--target", target,
		"--repo", repo, "--base-commit", baseCommit, "--code-out", scratch)
	if code == 0 {
		t.Fatal("overwrote an existing scratch file")
	}
	requireGuardedError(t, diagnostic, "WRITE_OUTPUT")
	kept, err := os.ReadFile(scratch)
	if err != nil {
		t.Fatal(err)
	}
	if string(kept) != "B\n" {
		t.Fatalf("scratch content changed: %q", kept)
	}
}

// Dry-run must validate the candidate without writing, so a model can check a
// coupled edit before writing it. The plan bytes must be identical after.
func TestDryRunLeavesPlanUnwritten(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "proposed.txt")
	view := revisionInspect(t, name, repo, baseCommit, target, "--code-out", scratch)
	if err := os.WriteFile(scratch, []byte("D\n"), 0600); err != nil {
		t.Fatal(err)
	}
	before, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	result := revisionPatch(t, name, target, view.EditExpect, repo, baseCommit,
		"--after-file", scratch, "--dry-run")
	if result.Written {
		t.Fatal("dry-run reported a write")
	}
	after, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatal("dry-run wrote a plan")
	}
}

// --diff prints a Git-generated preview for human review. It must show the new
// line so a reviewer can approve the change without opening the plan.
func TestPatchDiffPreviewShowsChange(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "proposed.txt")
	view := revisionInspect(t, name, repo, baseCommit, target, "--code-out", scratch)
	if err := os.WriteFile(scratch, []byte("D\n"), 0600); err != nil {
		t.Fatal(err)
	}
	code, out, diagnostic := revisionExecute(patchArgs(name, target, view.EditExpect,
		repo, baseCommit, "--after-file", scratch, "--dry-run", "--diff")...)
	if code != 0 {
		t.Fatalf("dry-run preview: exit %d: %s", code, diagnostic)
	}
	if !strings.Contains(out, "+D") {
		t.Fatalf("preview missing changed line: %q", out)
	}
}

// Applying a prefix uses the base commit plus every change through the edited
// one, never later changes. An earlier change must patch even when a later
// change is a PLACEHOLDER; check --repo --base-commit then reports the later
// target, and editing that change in order makes check pass.
func TestPrefixApplyIgnoresLaterBrokenChange(t *testing.T) {
	repo, baseCommit, name := revisionBrokenSecondFixture(t)
	first := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "first.txt")
	view := revisionInspect(t, name, repo, baseCommit, first, "--code-out", scratch)
	if err := os.WriteFile(scratch, []byte("D\n"), 0600); err != nil {
		t.Fatal(err)
	}
	result := revisionPatch(t, name, first, view.EditExpect, repo, baseCommit,
		"--after-file", scratch)
	if !result.PrefixReplayed || result.DownstreamChecked {
		t.Fatalf("prefix apply flags wrong: %+v", result)
	}
	code, _, diagnostic := revisionExecute("check", name, "--repo", repo, "--base-commit", baseCommit)
	if code == 0 {
		t.Fatal("broken later change passed the readiness check")
	}
	payload := requireGuardedError(t, diagnostic, planpatch.CodePatchInvalid)
	if !strings.Contains(payload.Message, "implementation[2].file_changes[1]") {
		t.Fatalf("check lost later target context: %q", payload.Message)
	}
	second := "implementation[2].file_changes[1]"
	scratch = filepath.Join(t.TempDir(), "second.txt")
	beforeView := revisionInspect(t, name, repo, baseCommit, second,
		"--code-out", scratch, "--before")
	current, err := os.ReadFile(scratch)
	if err != nil {
		t.Fatal(err)
	}
	if string(current) != "D\n" {
		t.Fatalf("wrong new prefix state: %q", current)
	}
	if err := os.WriteFile(scratch, []byte("E\n"), 0600); err != nil {
		t.Fatal(err)
	}
	revisionPatch(t, name, second, beforeView.EditExpect, repo, baseCommit,
		"--after-file", scratch)
	code, _, diagnostic = revisionExecute("check", name, "--repo", repo, "--base-commit", baseCommit)
	if code != 0 {
		t.Fatalf("completed revision failed: %s", diagnostic)
	}
}

// Callers use PATCH_NO_CHANGE to tell "you supplied no real change" apart from a
// malformed patch, which is PATCH_INVALID. A no-op replacement must return the
// first code and leave the plan untouched.
func TestUnchangedScratchReportsNoChange(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "proposed.txt")
	view := revisionInspect(t, name, repo, baseCommit, target,
		"--code-out", scratch, "--before")
	before, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	code, _, diagnostic := revisionExecute(patchArgs(name, target, view.EditExpect,
		repo, baseCommit, "--after-file", scratch)...)
	if code == 0 {
		t.Fatal("no-op replacement was accepted")
	}
	requireGuardedError(t, diagnostic, planpatch.CodePatchNoChange)
	after, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(before, after) {
		t.Fatal("no-op patch modified the plan")
	}
}

func TestUnchangedExportReportsNoChange(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "after.txt")
	view := revisionInspect(t, name, repo, baseCommit, target, "--code-out", scratch)
	planBefore, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	result := revisionPatch(t, name, target, view.EditExpect, repo, baseCommit,
		"--after-file", scratch)
	if result.Changed || result.Written {
		t.Fatalf("no-op result = %+v, want unchanged and unwritten", result)
	}
	planAfter, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(planBefore, planAfter) {
		t.Fatal("no-op changed the plan")
	}

	code, out, diagnostic := revisionExecute(patchArgs(name, target,
		view.EditExpect, repo, baseCommit, "--after-file", scratch,
		"--dry-run", "--diff")...)
	if code != 0 {
		t.Fatalf("no-op diff preview: exit %d: %s", code, diagnostic)
	}
	if out != "No changes.\n" {
		t.Fatalf("no-op preview = %q, want explicit empty preview", out)
	}
}

func TestPlaceholderRejectsUnchangedExport(t *testing.T) {
	repo, baseCommit, name := revisionBrokenSecondFixture(t)
	target := "implementation[2].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "before.txt")
	view := revisionInspect(t, name, repo, baseCommit, target,
		"--before", "--code-out", scratch)
	planBefore, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	code, _, diagnostic := revisionExecute(patchArgs(name, target,
		view.EditExpect, repo, baseCommit, "--after-file", scratch)...)
	if code == 0 {
		t.Fatal("unchanged before-state export was accepted")
	}
	if !strings.Contains(diagnostic,
		"after-file is identical to the --before file from inspect; edit it first") {
		t.Fatalf("error does not explain unchanged export: %s", diagnostic)
	}
	planAfter, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Equal(planBefore, planAfter) {
		t.Fatal("rejected before-state patch changed the plan")
	}
}

func TestInspectPrintsRawEditToken(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	view := revisionInspect(t, name, repo, baseCommit, target)
	code, out, diagnostic := revisionExecute("inspect", name,
		"--target", target, "--repo", repo, "--base-commit", baseCommit,
		"--print", "edit_expect")
	if code != 0 {
		t.Fatalf("print edit token: exit %d: %s", code, diagnostic)
	}
	if out != view.EditExpect+"\n" {
		t.Fatalf("printed token = %q, want raw token", out)
	}
	if view.Selector != target || view.Diff == "" || view.EditExpect == "" {
		t.Fatalf("default inspect JSON fields changed: %+v", view)
	}
}

func TestNewPlaceholderBeforeIsRejected(t *testing.T) {
	repo, baseCommit := revisionRepo(t, 0644)
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "new.txt"
	plan.Implementation[0].FileChanges[0].Diff = "PLACEHOLDER"
	plan.DefinitionOfDone.CurrentState = "Base commit: " + baseCommit
	name := writeRenderedPlan(t, plan)
	target := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "before.txt")
	view := revisionInspect(t, name, repo, baseCommit, target,
		"--before", "--code-out", scratch)
	if view.CodeExists == nil || *view.CodeExists {
		t.Fatalf("code_exists = %v, want false for new file", view.CodeExists)
	}
	data, err := os.ReadFile(scratch)
	if err != nil {
		t.Fatal(err)
	}
	if len(data) != 0 {
		t.Fatalf("before export for missing file = %q, want empty", data)
	}
	code, _, diagnostic := revisionExecute(patchArgs(name, target,
		view.EditExpect, repo, baseCommit, "--after-file", scratch)...)
	if code == 0 || !strings.Contains(diagnostic,
		"after-file is identical to the --before file from inspect; edit it first") {
		t.Fatalf("empty before-state export result: exit %d: %s", code, diagnostic)
	}
}

// Binary replacement bytes have no valid source diff, so Generate must report
// PATCH_UNSUPPORTED (not PATCH_INVALID) and leave the plan untouched.
func TestBinaryScratchIsUnsupported(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "proposed.txt")
	view := revisionInspect(t, name, repo, baseCommit, target, "--code-out", scratch)
	if err := os.WriteFile(scratch, []byte("A\x00B\n"), 0600); err != nil {
		t.Fatal(err)
	}
	code, _, diagnostic := revisionExecute(patchArgs(name, target, view.EditExpect,
		repo, baseCommit, "--after-file", scratch)...)
	if code == 0 {
		t.Fatal("binary replacement was accepted")
	}
	requireGuardedError(t, diagnostic, planpatch.CodePatchUnsupported)
}

// Guarded failures use the same {code, message, recovery_hint} JSON shape as
// other planner errors. Agents read code to decide what to do next, so the
// message must not repeat it and the hint must not be empty.
func TestGuardedErrorJSONShape(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	diff := filepath.Join(t.TempDir(), "change.diff")
	raw := "--- a/foo.txt\n+++ b/foo.txt\n@@ -1 +1 @@\n-A\n+B\n"
	if err := os.WriteFile(diff, []byte(raw), 0600); err != nil {
		t.Fatal(err)
	}
	code, _, diagnostic := revisionExecute(patchArgs(name, target,
		"sha256:deadbeef", repo, baseCommit, "--diff-file", diff)...)
	if code == 0 {
		t.Fatal("stale token was accepted")
	}
	payload := requireGuardedError(t, diagnostic, planpatch.CodePlanStale)
	if strings.Contains(payload.Message, payload.Code) {
		t.Fatalf("message repeats code: %q", payload.Message)
	}
	if payload.RecoveryHint == "" {
		t.Fatal("recovery_hint is empty")
	}
}

// Each options type validates itself, so the operation functions must reject bad
// combinations with USAGE before reading anything. Testing through inspect and
// patch keeps that rule independent of the CLI flag grammar; check defaults its
// repo and base commit, so it has no required pair to reject.
func TestGuardedOptionsRejectBadCombinations(t *testing.T) {
	const target = "implementation[1].file_changes[1]"
	baseCommit := strings.Repeat("a", 40)
	cases := []struct {
		name     string
		run      func() error
		wantCode string
	}{
		{"inspect missing target", func() error {
			_, err := guardedInspect(guardedInspectOptions{Repo: "repo", BaseCommit: baseCommit})
			return err
		}, "USAGE"},
		{"inspect missing repo and base commit", func() error {
			_, err := guardedInspect(guardedInspectOptions{Target: target})
			return err
		}, "USAGE"},
		{"inspect before without code-out", func() error {
			_, err := guardedInspect(guardedInspectOptions{
				Target: target, Repo: "repo", BaseCommit: baseCommit, Before: true,
			})
			return err
		}, "USAGE"},
		{"patch missing target", func() error {
			_, err := guardedPatch(guardedPatchOptions{
				Expect: "sha256:x", AfterFile: "a", Repo: "repo", BaseCommit: baseCommit,
			})
			return err
		}, "USAGE"},
		{"patch missing expect", func() error {
			_, err := guardedPatch(guardedPatchOptions{
				Target: target, AfterFile: "a", Repo: "repo", BaseCommit: baseCommit,
			})
			return err
		}, "USAGE"},
		{"patch missing repo and base commit", func() error {
			_, err := guardedPatch(guardedPatchOptions{
				Target: target, Expect: "sha256:x", AfterFile: "a",
			})
			return err
		}, "USAGE"},
		{"patch no replacement source", func() error {
			_, err := guardedPatch(guardedPatchOptions{
				Target: target, Expect: "sha256:x", Repo: "repo", BaseCommit: baseCommit,
			})
			return err
		}, "USAGE"},
		{"patch both replacement sources", func() error {
			_, err := guardedPatch(guardedPatchOptions{
				Target: target, Expect: "sha256:x",
				AfterFile: "a", DiffFile: "b", Repo: "repo", BaseCommit: baseCommit,
			})
			return err
		}, "USAGE"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			err := tc.run()
			if err == nil {
				t.Fatal("accepted invalid options")
			}
			var patchErr *planpatch.Error
			if !errors.As(err, &patchErr) {
				t.Fatalf("error does not carry a code: %v", err)
			}
			if patchErr.Code != tc.wantCode {
				t.Fatalf("code = %q, want %q", patchErr.Code, tc.wantCode)
			}
		})
	}
}

// inspect and patch always need the repository and the base commit, so a
// missing --repo or --base-commit exits 2 with USAGE. Without a base commit the
// edit token could not identify the source the change was prepared against.
func TestGuardedCommandsRequireBaseCommit(t *testing.T) {
	_, _, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	cases := []struct {
		name string
		args []string
	}{
		{"inspect", []string{"inspect", name, "--target", target}},
		{"patch", []string{"patch", name, "--target", target, "--expect", "sha256:x",
			"--after-file", os.DevNull}},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			code, _, diagnostic := revisionExecute(tc.args...)
			if code != 2 {
				t.Fatalf("exit = %d, want 2: %s", code, diagnostic)
			}
			requireGuardedError(t, diagnostic, "USAGE")
		})
	}
}

// --base-commit must override the recorded line, for example when re-pointing a
// plan at a different base commit whose recorded value is no longer present.
func TestCheckBaseCommitFlagOverridesPlan(t *testing.T) {
	repo, baseCommit, name := checkPlanFixture(t)
	raw, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	wrong := strings.Repeat("a", 40)
	updated := strings.Replace(string(raw), "Base commit: "+baseCommit,
		"Base commit: "+wrong, 1)
	if updated == string(raw) {
		t.Fatal("test setup did not replace the recorded base commit line")
	}
	if err := os.WriteFile(name, []byte(updated), 0o644); err != nil {
		t.Fatal(err)
	}

	code, out, diagnostic := revisionExecute("check", name, "--repo", repo, "--base-commit", baseCommit)
	if code != 0 {
		t.Fatalf("exit=%d want 0: %s", code, diagnostic)
	}
	result := decodeGuardedResult[guardedCheckResult](t, out)
	if result.BaseCommit != baseCommit {
		t.Fatalf("base commit=%q want %q", result.BaseCommit, baseCommit)
	}
}

// check must work from inside the repository without repeating --repo, so the
// default repo is the current working directory's Git repository.
func TestCheckDefaultsRepoToCwd(t *testing.T) {
	repo, baseCommit, name := checkPlanFixture(t)
	chdir(t, repo)

	code, out, diagnostic := revisionExecute("check", name)
	if code != 0 {
		t.Fatalf("exit=%d want 0: %s", code, diagnostic)
	}
	result := decodeGuardedResult[guardedCheckResult](t, out)
	if result.BaseCommit != baseCommit {
		t.Fatalf("base commit=%q want %q", result.BaseCommit, baseCommit)
	}
}

// A plan without a recorded base commit and no --base-commit must fail with a
// hint to add the line, never silently fall back to HEAD.
func TestCheckRequiresBaseCommit(t *testing.T) {
	repo, _, name := revisionFixture(t, false)

	code, _, diagnostic := revisionExecute("check", name, "--repo", repo)
	if code != 1 {
		t.Fatalf("exit=%d want 1: %s", code, diagnostic)
	}
	payload := requireGuardedError(t, diagnostic, codeBaseCommitRequired)
	if !strings.Contains(payload.RecoveryHint, "Base commit") {
		t.Fatalf("recovery hint %q missing Base commit", payload.RecoveryHint)
	}
}

// A PLACEHOLDER diff must fail a bare check. The original bug was that plain
// check printed OK here, so an unapplicable plan reached review.
func TestCheckRejectsBrokenDiff(t *testing.T) {
	repo, baseCommit := revisionRepo(t, 0644)
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "foo.txt"
	plan.Implementation[0].FileChanges[0].Diff = "PLACEHOLDER"
	plan.DefinitionOfDone.CurrentState = "Base commit: " + baseCommit
	name := writeRenderedPlan(t, plan)
	chdir(t, repo)

	code, _, diagnostic := revisionExecute("check", name)
	if code == 0 {
		t.Fatal("PLACEHOLDER diff passed check")
	}
	payload := requireGuardedError(t, diagnostic, planpatch.CodePatchInvalid)
	want := "implementation[1].file_changes[1] (foo.txt) is still PLACEHOLDER; " +
		"fill it with inspect --before and patch"
	if payload.Message != want {
		t.Fatalf("placeholder error = %q, want %q", payload.Message, want)
	}
}

func TestPatchNamesEarlierPlaceholder(t *testing.T) {
	repo, baseCommit := revisionRepo(t, 0644)
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "foo.txt"
	plan.Implementation[0].FileChanges[0].Diff = "PLACEHOLDER"
	plan.Implementation = append(plan.Implementation, Step{
		Title:   "Second edit",
		Summary: "Follow the unfinished change.",
		FileChanges: []FileChange{{
			Filename:    "foo.txt",
			Explanation: "Replace the source line.",
			Diff:        fooDiff(t, "A\n", "B\n", "100644"),
		}},
	})
	plan.DefinitionOfDone.CurrentState = "Base commit: " + baseCommit
	name := writeRenderedPlan(t, plan)
	target := "implementation[2].file_changes[1]"
	view := revisionInspect(t, name, repo, baseCommit, target)
	code, _, diagnostic := revisionExecute(patchArgs(name, target,
		view.EditExpect, repo, baseCommit, "--after-file", os.DevNull)...)
	if code == 0 {
		t.Fatal("patch apply accepted an earlier placeholder")
	}
	payload := requireGuardedError(t, diagnostic, planpatch.CodePatchInvalid)
	want := "implementation[1].file_changes[1] (foo.txt) is still PLACEHOLDER; " +
		"fill it with inspect --before and patch"
	if payload.Message != want {
		t.Fatalf("placeholder error = %q, want %q", payload.Message, want)
	}
}

// Selectors are parsed as numbers, so implementation[01] and implementation[1]
// pick the same change. Inspect echoes the normalized selector, and patch
// accepts either spelling because the edit token is built from the normalized
// one.
func TestLeadingZeroSelectorNormalizesAndPatches(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	leading := "implementation[01].file_changes[1]"
	normalized := "implementation[1].file_changes[1]"
	view := revisionInspect(t, name, repo, baseCommit, leading)
	if view.Selector != normalized {
		t.Fatalf("selector = %q, want %q", view.Selector, normalized)
	}
	raw, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	if want := planpatch.Expect(raw, normalized, baseCommit); view.EditExpect != want {
		t.Fatalf("token not bound to normalized selector")
	}
	scratch := filepath.Join(t.TempDir(), "proposed.txt")
	if err := os.WriteFile(scratch, []byte("D\n"), 0600); err != nil {
		t.Fatal(err)
	}
	result := revisionPatch(t, name, leading, view.EditExpect, repo, baseCommit,
		"--after-file", scratch)
	if !result.Written {
		t.Fatalf("leading-zero selector did not patch: %+v", result)
	}
	code, _, diagnostic := revisionExecute("check", name, "--repo", repo, "--base-commit", baseCommit)
	if code != 0 {
		t.Fatalf("patched plan failed check: %s", diagnostic)
	}
}

func exportTreeFixture(t *testing.T) (repo, baseCommit, name string) {
	t.Helper()
	repo, _ = revisionRepo(t, 0755)
	tracked := filepath.Join(repo, "tracked.txt")
	if err := os.WriteFile(tracked, []byte("base tree\n"), 0644); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(repo, "foo-link")
	if err := os.Symlink("foo.txt", link); err != nil {
		t.Fatal(err)
	}
	revisionGit(t, repo, "add", "tracked.txt", "foo-link")
	revisionGit(t, repo, "-c", "user.name=Test", "-c", "user.email=test@example.invalid",
		"-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "add tracked file")
	baseCommit = revisionGit(t, repo, "rev-parse", "HEAD")
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "foo.txt"
	plan.Implementation[0].FileChanges[0].Diff = fooDiff(t, "A\n", "B\n", "100755")
	plan.Implementation = append(plan.Implementation, Step{
		Title: "Second edit", Summary: "Complete the proposed change.",
		FileChanges: []FileChange{{Filename: "foo.txt", Explanation: "Finish the edit.",
			Diff: fooDiff(t, "B\n", "C\n", "100755")}},
	})
	deleteDiff, err := planpatch.Generate("tracked.txt",
		&planpatch.File{Data: []byte("base tree\n"), Mode: "100644"}, nil)
	if err != nil {
		t.Fatal(err)
	}
	plan.Implementation = append(plan.Implementation, Step{
		Title: "Remove tracked file", Summary: "Delete a base file.",
		FileChanges: []FileChange{{Filename: "tracked.txt", Explanation: "Remove this file.",
			Diff: strings.TrimSuffix(string(deleteDiff), "\n")}},
	})
	plan.DefinitionOfDone.CurrentState = "Base commit: " + baseCommit
	name = writeRenderedPlan(t, plan)
	return repo, baseCommit, name
}

func TestExportThroughStopsAfterStep(t *testing.T) {
	repo, baseCommit, name := exportTreeFixture(t)
	through := filepath.Join(t.TempDir(), "through")
	code, _, diagnostic := revisionExecute("export", name, "--repo", repo,
		"--base-commit", baseCommit, "--out", through, "--through", "1")
	if code != 0 {
		t.Fatalf("through export: %s", diagnostic)
	}
	got, err := os.ReadFile(filepath.Join(through, "foo.txt"))
	if err != nil || string(got) != "B\n" {
		t.Fatalf("step 1 source = %q, err=%v", got, err)
	}
	if info, err := os.Stat(filepath.Join(through, "foo.txt")); err != nil || info.Mode().Perm()&0111 == 0 {
		t.Fatalf("step 1 executable mode lost: info=%v err=%v", info, err)
	}
	if got, err := os.ReadFile(filepath.Join(through, "tracked.txt")); err != nil || string(got) != "base tree\n" {
		t.Fatalf("unmodified base file = %q, err=%v", got, err)
	}
	linkInfo, err := os.Lstat(filepath.Join(through, "foo-link"))
	if err != nil || linkInfo.Mode()&os.ModeSymlink == 0 {
		t.Fatalf("exported symlink entry = %v, err=%v", linkInfo, err)
	}
	if target, err := os.Readlink(filepath.Join(through, "foo-link")); err != nil || target != "foo.txt" {
		t.Fatalf("exported symlink target = %q, err=%v", target, err)
	}

}

func TestExportWritesWholePlanTree(t *testing.T) {
	repo, baseCommit, name := exportTreeFixture(t)
	full := filepath.Join(t.TempDir(), "full")
	code, _, diagnostic := revisionExecute("export", name, "--repo", repo,
		"--base-commit", baseCommit, "--out", full)
	if code != 0 {
		t.Fatalf("full export: %s", diagnostic)
	}
	got, err := os.ReadFile(filepath.Join(full, "foo.txt"))
	if err != nil || string(got) != "C\n" {
		t.Fatalf("full source = %q, err=%v", got, err)
	}
	if _, err := os.Lstat(filepath.Join(full, "tracked.txt")); !os.IsNotExist(err) {
		t.Fatalf("planned deletion remains in full export: %v", err)
	}
	if source, err := os.ReadFile(filepath.Join(repo, "foo.txt")); err != nil || string(source) != "A\n" {
		t.Fatalf("export changed source repository: %q, err=%v", source, err)
	}
}

func TestExportRefusesExistingOutput(t *testing.T) {
	repo, baseCommit := revisionRepo(t, 0644)
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "foo.txt"
	plan.Implementation[0].FileChanges[0].Diff = "PLACEHOLDER"
	plan.DefinitionOfDone.CurrentState = "Base commit: " + baseCommit
	name := writeRenderedPlan(t, plan)

	existing := filepath.Join(t.TempDir(), "existing")
	if err := os.Mkdir(existing, 0755); err != nil {
		t.Fatal(err)
	}
	sentinel := filepath.Join(existing, "sentinel")
	if err := os.WriteFile(sentinel, []byte("keep"), 0644); err != nil {
		t.Fatal(err)
	}
	code, _, diagnostic := revisionExecute("export", name, "--repo", repo,
		"--base-commit", baseCommit, "--out", existing)
	if code == 0 {
		t.Fatal("existing output path was accepted")
	}
	if !strings.Contains(diagnostic, "--out "+existing+" already exists; choose a new directory") {
		t.Fatalf("existing output error = %s", diagnostic)
	}
	if got, err := os.ReadFile(sentinel); err != nil || string(got) != "keep" {
		t.Fatalf("existing output changed: %q, err=%v", got, err)
	}
}

func TestFailedExportLeavesNoOutput(t *testing.T) {
	repo, baseCommit := revisionRepo(t, 0644)
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "foo.txt"
	plan.Implementation[0].FileChanges[0].Diff = "PLACEHOLDER"
	plan.DefinitionOfDone.CurrentState = "Base commit: " + baseCommit
	name := writeRenderedPlan(t, plan)
	missing := filepath.Join(t.TempDir(), "failed")
	code, _, _ := revisionExecute("export", name, "--repo", repo,
		"--base-commit", baseCommit, "--out", missing)
	if code == 0 {
		t.Fatal("unapplicable plan was exported")
	}
	if _, err := os.Lstat(missing); !os.IsNotExist(err) {
		t.Fatalf("failed export left output directory: %v", err)
	}
}

func TestExportRejectsInvalidThrough(t *testing.T) {
	cases := []struct {
		name string
		arg  string
	}{
		{"zero", "0"},
		{"too large", "9"},
		{"not a number", "abc"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			repo, baseCommit, name := revisionFixture(t, true)
			out := filepath.Join(t.TempDir(), "output")
			code, _, diagnostic := revisionExecute("export", name, "--repo", repo,
				"--base-commit", baseCommit, "--out", out, "--through", tc.arg)
			if code != 2 {
				t.Fatalf("exit = %d, want 2: %s", code, diagnostic)
			}
			if !strings.Contains(diagnostic, "--through must be a step number from 1 to 2") {
				t.Fatalf("through error = %s", diagnostic)
			}
			if _, err := os.Lstat(out); !os.IsNotExist(err) {
				t.Fatalf("invalid through left output: %v", err)
			}
		})
	}
}

// /dev/null is the documented way to propose deleting a file. The generated
// diff must be a deletion, and a later inspect must report code_exists false so
// a caller can distinguish deletion from an empty file.
func TestDeletionViaDevNullAfterFile(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	view := revisionInspect(t, name, repo, baseCommit, target)
	revisionPatch(t, name, target, view.EditExpect, repo, baseCommit,
		"--after-file", os.DevNull)
	after := revisionInspect(t, name, repo, baseCommit, target)
	if !strings.Contains(after.Diff, "+++ /dev/null") {
		t.Fatalf("deletion diff missing /dev/null: %q", after.Diff)
	}
	scratch := filepath.Join(t.TempDir(), "deleted.txt")
	codeView := revisionInspect(t, name, repo, baseCommit, target, "--code-out", scratch)
	if codeView.CodeExists == nil || *codeView.CodeExists {
		t.Fatalf("code_exists = %v, want false", codeView.CodeExists)
	}
}

// Patch keeps an existing file's executable bit instead of resetting it to
// 100644. Mode loss is invisible in the diff text, so only this check catches it.
func TestExecutableModeRetainedThroughPatch(t *testing.T) {
	repo, baseCommit, name := revisionExecutableFixture(t)
	target := "implementation[1].file_changes[1]"
	scratch := filepath.Join(t.TempDir(), "proposed.txt")
	view := revisionInspect(t, name, repo, baseCommit, target, "--code-out", scratch)
	if view.Mode != "100755" {
		t.Fatalf("inspect mode = %q, want 100755", view.Mode)
	}
	if err := os.WriteFile(scratch, []byte("C\n"), 0600); err != nil {
		t.Fatal(err)
	}
	revisionPatch(t, name, target, view.EditExpect, repo, baseCommit,
		"--after-file", scratch)
	again := filepath.Join(t.TempDir(), "after.txt")
	patched := revisionInspect(t, name, repo, baseCommit, target, "--code-out", again)
	if patched.Mode != "100755" {
		t.Fatalf("patched mode = %q, want 100755", patched.Mode)
	}
}

// --diff-file - imports a raw unified diff from stdin, which is how an external
// tool hands Planner a diff without a scratch file. The patch must apply the
// bytes exactly as if they came from a path.
func TestDiffFileFromStdin(t *testing.T) {
	repo, baseCommit, name := revisionFixture(t, false)
	target := "implementation[1].file_changes[1]"
	view := revisionInspect(t, name, repo, baseCommit, target)
	rawDiff, err := planpatch.Generate("foo.txt",
		&planpatch.File{Data: []byte("A\n"), Mode: "100644"},
		&planpatch.File{Data: []byte("C\n"), Mode: "100644"})
	if err != nil {
		t.Fatal(err)
	}
	var code int
	var out, diagnostic string
	withStdin(t, rawDiff, func() {
		code, out, diagnostic = revisionExecute(patchArgs(name, target,
			view.EditExpect, repo, baseCommit, "--diff-file", "-")...)
	})
	if code != 0 {
		t.Fatalf("stdin diff: exit %d: %s", code, diagnostic)
	}
	result := decodeGuardedResult[guardedPatchResult](t, out)
	if !result.Written || !result.PrefixReplayed {
		t.Fatalf("stdin diff result: %+v", result)
	}
	code, _, diagnostic = revisionExecute("check", name, "--repo", repo, "--base-commit", baseCommit)
	if code != 0 {
		t.Fatalf("patched plan failed check: %s", diagnostic)
	}
}

// A diff body can quote a line of backticks from the source. ReplaceDiff grows
// the fence so that line cannot close the block early and expose the rest of the
// replacement as other plan fields. If that growth stopped working, the edit
// would fail or change unrelated fields, so the note beside the block is checked.
func TestPatchKeepsBacktickContextInsideFence(t *testing.T) {
	// The original diff edits the last line, far from the backtick line, so it
	// gets a three-backtick fence. The replacement edits near the backtick line,
	// so its context contains a three-tick line that would close that fence.
	content := "```\na\nb\nc\nd\ne\nf\ng\n"
	repo, baseCommit := revisionRepoContent(t, content, 0644)
	plan := BuildPlanExample()
	plan.Implementation[0].FileChanges[0].Filename = "foo.txt"
	plan.Implementation[0].FileChanges[0].Diff = fooDiff(t, content,
		"```\na\nb\nc\nd\ne\nf\nG\n", "100644")
	name := writeRevisionPlan(t, plan)
	target := "implementation[1].file_changes[1]"
	view := revisionInspect(t, name, repo, baseCommit, target)
	rawDiff, err := planpatch.Generate("foo.txt",
		&planpatch.File{Data: []byte(content), Mode: "100644"},
		&planpatch.File{Data: []byte("```\nA\nb\nc\nd\ne\nf\ng\n"), Mode: "100644"})
	if err != nil {
		t.Fatal(err)
	}
	diff := filepath.Join(t.TempDir(), "fence.diff")
	if err := os.WriteFile(diff, rawDiff, 0600); err != nil {
		t.Fatal(err)
	}
	result := revisionPatch(t, name, target, view.EditExpect, repo, baseCommit,
		"--diff-file", diff)
	if !result.PatchSyntaxValid {
		t.Fatalf("backtick context rejected: %+v", result)
	}
	changed, err := os.ReadFile(name)
	if err != nil {
		t.Fatal(err)
	}
	if !bytes.Contains(changed, []byte("Human review note: keep this.")) {
		t.Fatal("collateral edit lost the review note")
	}
	code, _, diagnostic := revisionExecute("check", name, "--repo", repo, "--base-commit", baseCommit)
	if code != 0 {
		t.Fatalf("patched plan failed check: %s", diagnostic)
	}
}
