# Skill Routing Checks

Run these checks in fresh sessions after changing shared routing instructions,
skill descriptions, or planning workflows. After directly referenced context is
read, inspect skill calls before broader repository research, a review
conclusion, an edit, or a delegated task.

Set the model once for every row: `EVAL_MODEL="${EVAL_MODEL:-goog/qwen3.8}"`.
Pass it with `--model` to OpenCode and Pi/Vibe. Record an unavailable model as
unsupported rather than silently substituting a stronger model.

Before the planning check, allocate a unique output directory with
`mktemp -d "${TMPDIR:-/tmp}/skill-routing.XXXXXX"` and replace `<output-dir>`
below with the printed path. First install and compare the planning workflows as
described in `plan-workflows.md`; run the check in a fresh session.

| Harness | Request | Expected behavior |
| --- | --- | --- |
| OpenCode | `/create_plan In personal-dev-env, plan a Rust CLI test for ai/skills/rust-development/examples/src/main.rs and write it to <output-dir>/rust-plan.md.` | After reading the referenced file, load `behavior-focused-testing`, `code-documentation`, and `rust-development` before broader repository research. Name all three in any planning delegation; do not create a reviewer solely for skill routing. |
| OpenCode | `Review ai/skills/rust-development/examples/tests/cli.rs::missing_count_shows_usage without editing files.` | Load `behavior-focused-testing`, `code-documentation`, and `rust-development` before giving review findings. |
| OpenCode | `/create_plan In personal-dev-env, plan a Python test for ai/skills/python-development/examples/src/python_skill_examples/async_jobs.py and write it to <output-dir>/python-plan.md.` | After reading the referenced file, load `behavior-focused-testing`, `code-documentation`, and `python-development` before broader repository research. Name all three in any planning delegation. |
| OpenCode | `Review ai/skills/python-development/examples/tests/test_async_jobs.py::test_work_and_tasks_stay_bounded without editing files.` | Load `behavior-focused-testing`, `code-documentation`, and `python-development` before giving review findings. |
| OpenCode | `Explain what a Python list is.` | Do not load `python-development` for a basic syntax question with no planning, implementation, debugging, review, or test work. |
| OpenCode | `Update README wording only; do not review or change Go code.` | Do not load `go-development` or `code-documentation` merely because the repository contains source code. This also applies through the documentation workflow. |
| OpenCode | `Delegate a read-only review of ai/skills/rust-development/examples/tests/cli.rs::missing_count_shows_usage.` | Load the testing, code-documentation, and Rust skills, and name all three in the delegation prompt. |
| OpenCode | `Review vibe/src/adapters/docker.rs::prepare_provider_auth and its nearby source documentation without editing files.` | Load `code-documentation` and `rust-development` before giving findings. |
| OpenCode | `Review PR 253 without editing files.` | Load `review-plan` before giving review findings. |
| OpenCode | `Create a Zettel that captures the Rust CLI test review findings.` | Load `obsidian-zettel` before creating a vault note. |

Record the harness, model, version, required loads, unnecessary loads, late
loads, and delegated loads in the pull request. If a harness does not expose a
required event, record the check as unsupported rather than passed. These
checks cover routing, not the quality of the completed task; use
`plan-workflows.md` for completed-plan behavior.


## Delegated Skill Loading

These checks show whether a delegated agent, given only its task, loads the
right skills before it starts work. Run them before the change is merged, after
installing it from the pull request's worktree:

```bash
installer=$(mktemp /tmp/pde-installer.XXXXXX)
go build -C pde-installer -o "$installer" .
PDE_REPO_ROOT="$PWD" "$installer" install full
rg -l '^## When you are a delegated agent' ~/.agents/skills/*/SKILL.md
```

The last command lists five workflow skills. This install changes the skills
every session on this machine uses. After the merge, or if the pull request is
abandoned, run the same commands from an up-to-date checkout of main.

Start the session that delegates from the pull request's worktree, so
relative paths resolve there and agents do not see unrelated uncommitted work.
Delegate each prompt below once per model with the subagent tool, setting its
`model` field. Do not add an `Applicable skills:` line. Run the `goog/qwen3.8`
rows in their own batch, since they share the local LiteLLM server. A model
that cannot be reached is recorded as unsupported, not replaced. Subagents may
ask permission to use the temporary folders under `/tmp`; the person running
the checks approves them.

Models: `openai/gpt-5.6-luna`, `openai/gpt-6.1-sol`,
`claude-code/claude-opus-5-5[1m]`, `claude-code/claude-sonnet-5`,
`goog/qwen3.8`, and `opencode-go/deepseek-v4-pro`. On 2026-10-12,
`opencode-go/deepseek-v4-pro` answered "This Go model requires Global regions",
so expect to record it as unsupported. `goog/qwen3.8` is a Vibe worker model,
so it does not drive the implement prompt: delegate that prompt to
`openai/gpt-5.6-luna` with `openai-codex/gpt-5.6-luna` replaced by
`goog/qwen3.8`, and judge only the worker.

Replace `<output-dir>` with a new `mktemp -d` directory. For the implement
prompt, build a new fixture per model with the Disposable Fixture section of
`plan-workflows.md`. Run it with bash in a child process, so its shell options
stay out of your session, and print the values the prompt needs:

```bash
root=$(mktemp -d "${TMPDIR:-/tmp}/skill-routing.XXXXXX")
awk '/^## Disposable Fixture/{f=1} f&&/^````bash/{g=1;next} g&&/^````$/{exit} g' \
  ai/evals/plan-workflows.md > "$root.sh"
EVAL_ROOT="$root" bash -c '. "$1" >/dev/null; echo "$EVAL_REPO $EVAL_FIXTURE_COMMIT"' _ "$root.sh"
```

Replace `$EVAL_REPO` and `$EVAL_FIXTURE_COMMIT` with the printed values, and
`<key>` with a new Vibe key per model, such as `skill-eval-sonnet`: at most 48
characters matching `[a-z0-9]+(-[a-z0-9]+)*`. Do not use the fixture's
`EVAL_KEY`; fixtures built in the same second share a commit, so their
`EVAL_KEY` values repeat. Before delegating, `vibe status --key <key> --long`
run in `$EVAL_REPO` must report no runs. The fixture's `docs/late-change.md`
belongs to another check, so the prompt says to ignore it. These fixtures skip
the Install section and its cleanup trap; when the results are recorded,
remove each `$root`, `$root.sh`, and `<output-dir>`.

| Prompt | Expected skills |
| --- | --- |
| `Write an implementation plan for one new CLI test for ai/skills/rust-development/examples/src/main.rs that covers behavior tests/cli.rs does not cover yet. Write it to <output-dir>/rust-plan.md. Decisions: 1. orchestrator-proposed: add the test to tests/cli.rs and leave src/ unchanged.` | `create-plan`, `behavior-focused-testing`, `code-documentation`, `rust-development` |
| `Review this proposed Go change without editing files and report your findings:` then a blank line and the diff below, inside its `diff` code fence | `review-plan`, `go-development`, `code-documentation`, `behavior-focused-testing` |
| `Implement only step 2 of $EVAL_REPO/plans/implement.md through Vibe. Base commit: $EVAL_FIXTURE_COMMIT. Ignore docs/late-change.md; it belongs to another check. The user allows Vibe key <key> and its local managed commits; the worker model is openai-codex/gpt-5.6-luna. Do not push, open a pull request, or merge.` | Driver: `implement-plan`, `go-development`. Vibe worker: `code-documentation` and each skill its `Applicable skills:` line names; the line names no workflow skill |

The review diff has a planted bug: `Truncate` returns one byte fewer than
asked, and its test does not catch it.

```diff
--- /dev/null
+++ b/internal/output/truncate.go
@@ -0,0 +1,9 @@
+package output
+
+// Truncate shortens value to at most max bytes for narrow terminals.
+func Truncate(value string, max int) string {
+	if len(value) <= max {
+		return value
+	}
+	return value[:max-1]
+}
--- /dev/null
+++ b/internal/output/truncate_test.go
@@ -0,0 +1,9 @@
+package output
+
+import "testing"
+
+func TestTruncateKeepsShortValues(t *testing.T) {
+	if got := Truncate("abc", 5); got != "abc" {
+		t.Fatalf("Truncate() = %q, want %q", got, "abc")
+	}
+}
```

For each run, pass the subagent's session ID to the audit. For the implement
prompt, also pass the Vibe run folder: `artifacts_dir` in the JSON that
`vibe status --key <key> --long` prints when run in `$EVAL_REPO`. It names the
latest run; earlier runs for the key are beside it.

```bash
python3 ai/evals/skill_audit.py <session-id> --vibe <run-dir>
```

It prints the skills read before the first edit, the skills first read after
it, and whether `git push` ran. A run passes when every expected skill is in
`before=` and `push=no`. For the Vibe worker, the expected skills come from
the `Applicable skills:` line in the run's `prompt.txt`; a missing line fails
the worker.

If a model misses a skill, delegate the same prompt once more to that model
with an `Applicable skills:` line naming the expected skills, and record both
runs. Record one row per model and prompt in the pull request, without session
IDs or local paths:

| Model | Prompt | Audit output | Result |
| --- | --- | --- | --- |
| `openai/gpt-5.6-luna` | plan | `before=... after=- push=no` | pass, fail, rerun pass, or unsupported |
