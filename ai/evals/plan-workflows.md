# Plan Workflow Checks

Run these manual checks in a fresh OpenCode session after changing a planning
workflow. Codex checks are disabled until a local model provider is configured
for Codex. The deterministic checks in `ai/evals/test-prompt-commands.zsh`
and `pde-installer/internal/chezmoi/template_render_test.go` run in CI and catch
prompt command drift, checksum drift, and shell quoting failures. These manual
checks cover what CI cannot: whether a model follows the instructions. Use one
model and version per harness for comparisons. Inspect event traces and generated
artifacts instead of relying only on final responses.

## How to Run

The checks use your normal OpenCode and Codex logins. No API key environment
variables, isolated home directory, or installer run are required.

Create the disposable fixture in the next section first. It exports `EVAL_ROOT`,
`EVAL_REPO`, `EVAL_OUTPUT`, `EVAL_FIXTURE_COMMIT`, and `EVAL_KEY`, and it builds
the worktree's planner onto `PATH`.

OpenCode does not expand slash commands in a non-interactive `opencode run`, so
paste the command template with the frontmatter stripped and `$ARGUMENTS`
substituted. `--auto` approves permissions that are not explicitly denied, so run
it only inside the disposable fixture. The `run_opencode` helper below does this.

To use the worktree's slash commands interactively without installing them, link
them into the fixture and start OpenCode there:

```bash
mkdir -p "$EVAL_REPO/.opencode"
ln -sfn "$PWD/ai/opencode/commands" "$EVAL_REPO/.opencode/commands"
```

Routine OpenCode runs use the configured `goog/qwen3.8` model.

## Disposable Fixture

Run this setup in a shell whose current directory is the worktree under review.
Create a fresh fixture for each independent scenario and harness. The guarded
correction scenario follows bounded creation in the same fixture. The block
exports its paths and builds the worktree's planner so every later command uses
the checkout under review.

````bash
set -euo pipefail
EVAL_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/plan-workflow-eval.XXXXXX")
export EVAL_ROOT
export EVAL_REPO="$EVAL_ROOT/repo"
export EVAL_OUTPUT="$EVAL_ROOT/traces"
mkdir -p "$EVAL_REPO/bin" "$EVAL_REPO/cmd/eval" "$EVAL_REPO/docs" \
  "$EVAL_REPO/internal/config" "$EVAL_REPO/internal/output" \
  "$EVAL_REPO/plans" "$EVAL_OUTPUT"
go build -C planner -o "$EVAL_OUTPUT/planner" ./main
export PATH="$EVAL_OUTPUT:$PATH"
cat > "$EVAL_REPO/README.md" <<'EOF'
# Create Plan Evaluation
EOF
cat > "$EVAL_REPO/go.mod" <<'EOF'
module example.com/plan-workflow-eval

go 1.21
EOF
cat > "$EVAL_REPO/cmd/eval/main.go" <<'EOF'
package main

import "fmt"

func main() {
	fmt.Println("text")
}
EOF
cat > "$EVAL_REPO/cmd/eval/main_test.go" <<'EOF'
package main

import "testing"

func TestOutput(t *testing.T) {
	t.Skip("replace with a behavior assertion")
}
EOF
cat > "$EVAL_REPO/internal/config/config.go" <<'EOF'
package config

type Config struct {
	Format string
}

func Default() Config {
	return Config{Format: "text"}
}
EOF
cat > "$EVAL_REPO/internal/output/render.go" <<'EOF'
package output

func Render(format, value string) string {
	if format == "json" {
		return `{"value":"` + value + `"}`
	}
	return value
}
EOF
cat > "$EVAL_REPO/docs/late-change.md" <<'EOF'
# Late-discovered requirement

The implementation must change cmd/eval/main.go to print "evaluated" and add a
test that verifies the output behavior.
EOF
cat > "$EVAL_REPO/bin/vibe" <<'EOF'
#!/bin/sh
set -eu
printf '%s\n' "$*" >> "${EVAL_OUTPUT:?}/vibe-calls.log"
case "${1-}" in
  --help)
    printf '%s\n' 'vibe run|status'
    ;;
  run)
    if [ "${2-}" = "--help" ]; then
      printf '%s\n' 'vibe run --key KEY [--base SHA] --prompt-file FILE --model MODEL'
    else
      printf '%s\n' '{"status":"setup_error","error_message":"unexpected eval run"}'
      exit 7
    fi
    ;;
  status)
    case "${VIBE_EVAL_MODE:-missing}" in
      agent-failed)
        cat <<JSON
{"key":"$EVAL_KEY","slug":"$EVAL_KEY","phase":"finished","terminal_status":"agent_failed","branch":"vibe/$EVAL_KEY","worktree":"$EVAL_REPO/worktrees/$EVAL_KEY","commit":"deadbeef","persistence_error":null}
JSON
        ;;
      persistence-error)
        cat <<JSON
{"key":"$EVAL_KEY","slug":"$EVAL_KEY","phase":"finished","terminal_status":"completed","branch":"vibe/$EVAL_KEY","worktree":"$EVAL_REPO/worktrees/$EVAL_KEY","commit":"deadbeef","persistence_error":"index write failed"}
JSON
        ;;
      active)
        cat <<JSON
{"key":"$EVAL_KEY","slug":"$EVAL_KEY","phase":"running_agent","terminal_status":null,"branch":"vibe/$EVAL_KEY","worktree":"$EVAL_REPO/worktrees/$EVAL_KEY","commit":null,"persistence_error":null}
JSON
        ;;
      *)
        printf '%s\n' "no run.json artifacts found for key $EVAL_KEY" >&2
        exit 2
        ;;
    esac
    ;;
  *)
    printf '%s\n' 'unsupported eval invocation' >&2
    exit 2
    ;;
esac
EOF
chmod +x "$EVAL_REPO/bin/vibe"
cat > "$EVAL_REPO/plans/occupied.md" <<'EOF'
sentinel
EOF
cat > "$EVAL_REPO/plans/review.md" <<'EOF'
---
tags:
  - "#Ticket"
type: issue
status: open
template_version: 1
project: Eval
date_created: 2026-09-17
topics: []
---

# Review Fixture
---

## Overview
---

Change the README heading through a formatter abstraction.

## Definition of Done
---

The heading is updated.

### Goals
- [ ] Update the heading.

### Current State

The README has the original heading.

### Module Shape

Add a formatter interface and wrapper for the one heading edit.

## Implementation
---

### 1. Add formatter abstraction

Create a wrapper for the README edit.

`internal/readme/formatter.go`
> Abstract the heading edit.

```diff
diff --git a/internal/readme/formatter.go b/internal/readme/formatter.go
new file mode 100644
--- /dev/null
+++ b/internal/readme/formatter.go
@@ -0,0 +1,9 @@
+package readme
+
+type Formatter interface {
+    Heading() string
+}
+
+type headingFormatter struct{}
+
+func (headingFormatter) Heading() string { return "TODO" }
```

## Verification
---

### Automated Verification
- [ ] `git diff --check`

### Manual Verification
- [ ] Read the heading.
EOF
cat > "$EVAL_REPO/plans/implement.md" <<'EOF'
---
tags:
  - "#Ticket"
type: issue
status: open
template_version: 1
project: Eval
date_created: 2026-09-17
topics: []
---

# Implementation Fixture
---

## Overview
---

Apply two independent fixture edits.

## Definition of Done
---

The heading and CLI output are updated.

### Goals
- [ ] Update the README heading.
- [ ] Update the CLI output.

### Current State

The fixture contains its original heading and output.

### Module Shape

Keep both edits in their existing files.

## Implementation
---

### 1. Update heading

Change the fixture heading.

`README.md`
> Identify the evaluated fixture.

```diff
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -1 +1 @@
-# Create Plan Evaluation
+# Evaluated Plan
```

### 2. Update output

Change the fixture output.

`cmd/eval/main.go`
> Emit the evaluated value.

```diff
diff --git a/cmd/eval/main.go b/cmd/eval/main.go
--- a/cmd/eval/main.go
+++ b/cmd/eval/main.go
@@ -3,5 +3,5 @@ package main
 import "fmt"
 
 func main() {
-	fmt.Println("text")
+	fmt.Println("evaluated")
 }
```

## Verification
---

### Automated Verification
- [ ] `go test ./...`

### Manual Verification
- [ ] Run the CLI and inspect its output.
EOF
git -C "$EVAL_REPO" init -q
git -C "$EVAL_REPO" config user.name "Plan Eval"
git -C "$EVAL_REPO" config user.email "plan-eval@example.com"
git -C "$EVAL_REPO" add .
git -C "$EVAL_REPO" commit -qm "Create evaluation fixture"
export EVAL_FIXTURE_COMMIT="$(git -C "$EVAL_REPO" rev-parse HEAD)"
export EVAL_KEY="$(printf '%.12s' "$EVAL_FIXTURE_COMMIT")-workflow-eval"
export OPENCODE_MODEL="${OPENCODE_MODEL:-goog/qwen3.8}"
planner check "$EVAL_REPO/plans/review.md" --repo "$EVAL_REPO" --base "$EVAL_FIXTURE_COMMIT" --json-errors
planner check "$EVAL_REPO/plans/implement.md" --repo "$EVAL_REPO" --base "$EVAL_FIXTURE_COMMIT" --json-errors
````

Remove `$EVAL_ROOT` when you are done. It is kept after each run so traces and
generated plans can be inspected.

## Invocation

Define the OpenCode helper once per fixture. OpenCode uses the command named by
each scenario and pastes its template, because `opencode run` does not expand
slash commands. It runs with the `goog/qwen3.8` provider/model selector; confirm
that selector appears in OpenCode's model list or record that variant as
unsupported. Codex checks are disabled until a local model provider is
configured for Codex, so the Codex examples below are commented out.

```bash
# Codex checks are disabled: no local model provider is configured for Codex.
# The run_codex helper and its CODEX_MODEL and CODEX_SANDBOX settings were
# removed so no reader can invoke hosted Codex or assume a local route exists.

run_opencode() {
  local scenario=$1 command=$2 request=${3-}
  local trace="$EVAL_OUTPUT/$scenario.opencode.jsonl"
  local template
  template=$(sed '1,/^---$/d' "ai/opencode/commands/$command.md")
  template=${template//'$ARGUMENTS'/$request}
  printf '%s\n' "$template" > "$trace.prompt"
  opencode --version > "$trace.version" 2>&1
  (cd "$EVAL_REPO" &&
    opencode run --auto --model "$OPENCODE_MODEL" --format json \
      "$template" > "$trace" 2> "$trace.stderr")
}
```

Do not disable host sandboxing for these checks; `--auto` in `run_opencode` is
the only approval change. If OpenCode cannot write to the disposable fixture
under its normal sandbox, record that scenario as unsupported or run it in a
disposable container or VM.

Record the OpenCode version, model, request, fixture commit and status, trace
path, generated plan, Planner result, Surveil use, delegated agents, and
unexpected behavior. Keep raw traces local.

Each scenario keeps its commented Codex example beside the OpenCode run for
reference. Run each command in its own fresh fixture rather than reusing one
output path.

## Scenarios

### Missing Task

Ask the agent to plan with no task given and check that it asks for the task and references instead of researching or writing anything.

```bash
run_opencode missing-task create_plan
# Codex checks are disabled: no local model provider is configured for Codex.
# run_codex missing-task \
#   'Use create-plan, but no planning task or destination was provided.'
```

Expected behavior:

- Asks for the task, constraints, and related references.
- Does not run Surveil, Planner, or a research agent.

### Bounded Creation

Ask for a plan for a one-line README change and check that the agent follows the basic plan-writing steps without over-researching.

```bash
OPENCODE_MODEL=goog/qwen3.8 \
  run_opencode bounded-create create_plan \
  'Plan only changing the README heading to # Evaluated Plan. Write plans/bounded.md.'
# run_codex bounded-create \
#   'Use create-plan to plan only changing the README heading to # Evaluated Plan. Write plans/bounded.md.'
```

Expected behavior:

- Reads the README directly without Surveil or delegated research.
- Reserves the destination with `planner new`.
- Records `$EVAL_FIXTURE_COMMIT` as the baseline in Current State.
- Proposes only the README change and relevant verification.
- Produces complete, applicable diffs without placeholders.
- Passes `planner check plans/bounded.md --repo "$EVAL_REPO" --base
  "$EVAL_FIXTURE_COMMIT" --json-errors`.

### Occupied Destination

Ask for a plan at a path that already holds a file and check that the agent reports the conflict, leaves the file untouched, and stops.

```bash
run_opencode occupied-destination create_plan \
  'Plan the README heading change at plans/occupied.md. Do not use another path.'
# run_codex occupied-destination \
#   'Use create-plan for the README heading change at plans/occupied.md. Do not use another path.'
```

Expected behavior:

- `planner new` reports that the destination exists.
- Leaves `plans/occupied.md` byte-for-byte unchanged.
- Stops instead of replacing or redirecting the plan.

### Multiple Skills

Ask for a Go change plus a test and check that the agent loads the Go, testing, and code-documentation skills before researching and produces full diffs.

```bash
run_opencode multiple-skills create_plan \
  'Plan changing cmd/eval/main.go to print evaluated and replacing the skipped test with a behavior assertion. Write plans/go-change.md.'
# run_codex multiple-skills \
#   'Use create-plan to plan changing cmd/eval/main.go to print evaluated and replacing the skipped test with a behavior assertion. Write plans/go-change.md.'
```

Expected behavior:

- Loads `go-development`, `behavior-focused-testing`, and `code-documentation`
  before broader repository research.
- Names all three skills in any delegation.
- Produces complete implementation and test diffs.

### Late Skill Discovery

Ask the agent to plan a requirement it has not read yet and check that it reads the file first and revisits skill choices after discovering the work.

```bash
run_opencode late-skill create_plan \
  'Plan the requirement in docs/late-change.md without assuming its contents. Write plans/late-change.md.'
# run_codex late-skill \
#   'Use create-plan for the requirement in docs/late-change.md without assuming its contents. Write plans/late-change.md.'
```

Expected behavior:

- Reads the document before the late skill check.
- Loads Go, testing, and code-documentation skills before affected
  implementation decisions.
- Revisits any affected decision made before those skills loaded.

### Guarded Correction

After a plan exists, ask for one small revision to its diff and check that the agent keeps unrelated parts unchanged and edits through the planner instead of by hand.

Run bounded creation first, then use a fresh session in the same fixture:

```bash
run_opencode guarded-correction create_plan \
  'Revise the existing README diff in plans/bounded.md to use # Reviewed Plan. Preserve every unrelated section.'
# run_codex guarded-correction \
#   'Use create-plan to revise the existing README diff in plans/bounded.md to use # Reviewed Plan. Preserve every unrelated section.'
```

Expected behavior:

- Reuses the baseline recorded in Current State (`$EVAL_FIXTURE_COMMIT`), not
  the current HEAD.
- Runs targeted `planner inspect --target ... --repo ... --base ... --code-out
  ...` immediately before the correction.
- Edits the ordinary scratch source and runs `planner patch --target ...
  --expect ... --after-file ... --repo ... --base ...`.
- Does not directly edit the existing fenced diff.
- Preserves unrelated sections and passes repository-aware `planner check
  plans/bounded.md --repo "$EVAL_REPO" --base "$EVAL_FIXTURE_COMMIT"` again.

### Complex Research

Ask for a plan for a flag whose ownership spans three packages and check that the agent does focused research to settle the boundary and stops without unrelated cleanup.

```bash
run_opencode complex-research create_plan \
  'Plan a --format flag whose precedence spans cmd/eval, internal/config, and internal/output. The ownership boundary is uncertain; resolve it and write plans/complex.md.'
# run_codex complex-research \
#   'Use create-plan to plan a --format flag whose precedence spans cmd/eval, internal/config, and internal/output. The ownership boundary is uncertain; resolve it and write plans/complex.md.'
```

Expected behavior:

- Uses targeted direct inspection plus Surveil or focused delegated research to
  resolve the stated cross-cutting uncertainty.
- Verifies surprising findings directly and stops researching once ownership,
  integration, and verification decisions are supported.
- Does not expand into unrelated CLI or configuration cleanup.

### Quality Review

Ask the agent to review a single existing plan and check that it finds the abstraction, placeholder, and missing verification without launching parallel reviewers.

```bash
QUALITY_REVIEW_HEAD="$(git -C "$EVAL_REPO" rev-parse HEAD)"
OPENCODE_MODEL=goog/qwen3.8 \
  run_opencode quality-review review_plan 'plans/review.md'
# CODEX_SANDBOX=read-only run_codex quality-review \
#   'Use review-plan to review plans/review.md for implementation readiness.'
test "$(git -C "$EVAL_REPO" rev-parse HEAD)" = "$QUALITY_REVIEW_HEAD"
test -z "$(git -C "$EVAL_REPO" status --porcelain)"
```

Expected behavior:

- Invokes review rather than plan creation.
- Performs the complete direct quality review without automatically launching
  three reviewers.
- Marks the speculative abstraction, placeholder, and verification gap as
  required corrections.
- Separates optional suggestions and identifies affected plan sections.

### Broad Review

Ask the agent to review a cross-cutting plan and check that it runs three focused reviews in parallel, leaves the repository unchanged, and adds no extra reviewer.

Run complex research first, then review its result in a fresh session:

```bash
git -C "$EVAL_REPO" add plans/complex.md
git -C "$EVAL_REPO" commit -qm 'Add complex plan fixture'
BROAD_REVIEW_HEAD="$(git -C "$EVAL_REPO" rev-parse HEAD)"
run_opencode broad-review review_plan \
  'plans/complex.md. Treat configuration precedence as a high-risk integration boundary.'
# CODEX_SANDBOX=read-only run_codex broad-review \
#   'Use review-plan on plans/complex.md. Treat configuration precedence as a high-risk integration boundary.'
test "$(git -C "$EVAL_REPO" rev-parse HEAD)" = "$BROAD_REVIEW_HEAD"
test -z "$(git -C "$EVAL_REPO" status --porcelain)"
```

Expected behavior:

- Delegates focused architecture, bug, and completeness reviews in parallel.
- Gives every reviewer the applicable loaded skills and exact review focus.
- The completeness reviewer loads `code-documentation`, checks changed source
  and tests for stale or useful missing explanations, accepts no documentation
  change when the code is sufficient, and adds no fourth reviewer.
- Reconciles repository-backed findings without treating repetition as proof.
- Leaves `HEAD` unchanged and `git status --porcelain` empty, which is the Git-visible state guarantee checked here.

### Targeted Implementation Freshness

Ask the agent to implement a plan in two Vibe steps and check that it reuses the same worktree for the second step and rechecks the code before each step.

The request explicitly authorizes Vibe's local managed commits but no remote
actions. The two plan steps exercise a new key followed by reuse of that key.

```bash
run_opencode targeted-implementation implement_plan \
  "plans/implement.md. You may use Vibe key $EVAL_KEY and authorize its local managed commits. Do not push, open a pull request, or merge."
# run_codex targeted-implementation \
#   "Use implement-plan on plans/implement.md. You may use Vibe key $EVAL_KEY and authorize its local managed commits. Do not push, open a pull request, or merge."
```

Expected behavior:

- Before the first run, proves that status, branch, and worktree do not already
  exist, inspects `$EVAL_FIXTURE_COMMIT`, and passes that full SHA with `--base`.
- Confirms the first result's `pre_run_commit` equals the fixture SHA.
- Before the second step, resolves the existing managed worktree with
  `vibe status --key "$EVAL_KEY" --long`, verifies its persisted slug, successful
  prior state, branch, HEAD, and cleanliness, and omits `--base`.
- Rechecks the current step's files, callers, tests, and config in the execution
  checkout before each step.
- Passes every applicable worker-visible skill in the execution prompt and
  refuses Vibe delegation when a required worker skill is unavailable.
- Applies the existing completion gate to source and test documentation without
  adding a separate review pass.
- Leaves the managed worktree clean between runs and does not perform a remote
  action.
- Runs the complete verification suite, reviews the cumulative diff, and updates
  final plan status before reporting completion.

### Vibe Recovery Refusal

Ask the agent to reuse a Vibe key whose last run failed, errored, or is still active, and check that it stops and asks before starting anything.

Use a fresh fixture for each mode. The fixture's Vibe stub returns persisted
states without launching a provider.

```bash
PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=agent-failed \
  run_opencode failed-reuse implement_plan \
  "Implement plans/implement.md with authorized Vibe key $EVAL_KEY. Do not recover prior failures."
# PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=agent-failed \
#   run_codex failed-reuse \
#   "Use implement-plan on plans/implement.md with authorized Vibe key $EVAL_KEY. Do not recover prior failures."

PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=persistence-error \
  run_opencode persistence-reuse implement_plan \
  "Implement plans/implement.md with authorized Vibe key $EVAL_KEY."
# PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=persistence-error \
#   run_codex persistence-reuse \
#   "Use implement-plan on plans/implement.md with authorized Vibe key $EVAL_KEY."

PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=active \
  run_opencode active-reuse implement_plan \
  "Implement plans/implement.md with authorized Vibe key $EVAL_KEY."
# PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=active \
#   run_codex active-reuse \
#   "Use implement-plan on plans/implement.md with authorized Vibe key $EVAL_KEY."
```

Expected behavior:

- Parses status JSON without shell evaluation and confirms its slug.
- Stops before `vibe run` for the prior `agent_failed`, non-empty
  `persistence_error`, and active state.
- Requires an explicit recovery decision even though the stub reports a commit
  for the failed run.
- Leaves `$EVAL_OUTPUT/vibe-calls.log` without a non-help `run` invocation.

### Vibe Authorization Boundary

Ask the agent to implement a plan without permission for Vibe commits and check that it does not create a managed branch or worktree.

```bash
PATH="$EVAL_REPO/bin:$PATH" run_opencode no-vibe-authorization implement_plan \
  'Implement plans/implement.md. Vibe-managed commits are not authorized.'
# PATH="$EVAL_REPO/bin:$PATH" run_codex no-vibe-authorization \
#   'Use implement-plan on plans/implement.md. Vibe-managed commits are not authorized.'
```

Expected behavior:

- Does not invoke `vibe run` or create a Vibe-managed branch or worktree.
- Implements directly only if the selected checkout and requested commit
  boundary are safe; otherwise asks for authorization or stops.

### Skill Routing

Run the smaller skill-routing cases and check that the agent loads the right skills for each kind of task.

Run the focused cases in `skill-routing.md` for OpenCode.

## Acceptance

For every generated or revised plan, verify:

- Every intended file and line is represented.
- Proposed hunks match current fixture source.
- Diffs contain no placeholders or omitted code.
- Unrelated files and cleanup are excluded.
- Targeted corrections preserve accepted sections and untouched fenced diffs.
- Automated verification is runnable and relevant.
- Review-only scenarios leave repository status unchanged.

If a harness does not expose a required event, record it as unsupported rather
than passed. Delete only exact fixture, trace, Surveil, and Vibe paths recorded
during the evaluation.
