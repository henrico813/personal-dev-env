# Plan Workflow Checks

Run these manual checks in fresh Codex and OpenCode sessions after changing a
planning workflow. Use one model and version per harness for comparisons.
Inspect event traces and generated artifacts instead of relying only on final
responses.

## Install

From the worktree under review:

```bash
set -euo pipefail
ORIGINAL_HOME=${HOME-}
ORIGINAL_PATH=${PATH-}
ORIGINAL_HOME_SET=${HOME+x}
ORIGINAL_PATH_SET=${PATH+x}
declare -A ORIGINAL_PROVIDER_VALUES=()
declare -A ORIGINAL_PROVIDER_SET=()
PROVIDER_VARIABLES=(
  ANTHROPIC_API_KEY OPENAI_API_KEY GEMINI_API_KEY DEEPSEEK_API_KEY
  AZURE_OPENAI_API_KEY AZURE_OPENAI_BASE_URL OPENCODE_API_KEY
)
for provider in "${PROVIDER_VARIABLES[@]}"; do
  ORIGINAL_PROVIDER_VALUES[$provider]=${!provider-}
  if [[ -v $provider ]]; then ORIGINAL_PROVIDER_SET[$provider]=1; else ORIGINAL_PROVIDER_SET[$provider]=0; fi
done
if [[ -z "${ORIGINAL_PROVIDER_VALUES[OPENCODE_API_KEY]}" || -z "${ORIGINAL_PROVIDER_VALUES[OPENAI_API_KEY]}" ]]; then
  printf '%s\n' 'Set both OPENCODE_API_KEY and OPENAI_API_KEY before running the evaluation' >&2
  exit 1
fi
EVAL_ROOT=$(mktemp -d "${TMPDIR:-/tmp}/plan-workflow-eval.XXXXXX")
export EVAL_ROOT
export EVAL_HOME="$EVAL_ROOT/home"
export EVAL_REPO="$EVAL_ROOT/repo"
export EVAL_OUTPUT="$EVAL_ROOT/traces"
INSTALLER="$EVAL_ROOT/pde-installer"
restore_environment() {
  if [[ -n "$ORIGINAL_HOME_SET" ]]; then export HOME="$ORIGINAL_HOME"; else unset HOME; fi
  if [[ -n "$ORIGINAL_PATH_SET" ]]; then export PATH="$ORIGINAL_PATH"; else unset PATH; fi
  for provider in "${PROVIDER_VARIABLES[@]}"; do
    if (( ORIGINAL_PROVIDER_SET[$provider] )); then
      export "$provider=${ORIGINAL_PROVIDER_VALUES[$provider]}"
    else
      unset "$provider"
    fi
  done
}
cleanup_evaluation() {
  local status=$?
  if [[ "$status" -eq 0 ]]; then
    if [[ -n "$ORIGINAL_PATH_SET" ]]; then PATH="$ORIGINAL_PATH"; else unset PATH; fi
    if ! rm -rf -- "$EVAL_ROOT"; then
      status=1
      printf '%s\n' "$EVAL_ROOT" >&2
    fi
  else
    printf '%s\n' "$EVAL_ROOT" >&2
  fi
  restore_environment
  exit "$status"
}
trap cleanup_evaluation EXIT
for provider in "${PROVIDER_VARIABLES[@]}"; do unset "$provider"; done
export HOME="$EVAL_HOME"
export PATH="$ORIGINAL_PATH"
mkdir -p "$EVAL_HOME" "$EVAL_REPO" "$EVAL_OUTPUT"
go build -C pde-installer -o "$INSTALLER" .
PDE_REPO_ROOT="$PWD" "$INSTALLER" install full
export PATH="$EVAL_HOME/.local/bin:$ORIGINAL_PATH"
cmp ai/codex/skills/create-plan/SKILL.md \
  "$HOME/.codex/skills/create-plan/SKILL.md"
cmp ai/codex/skills/review-plan/SKILL.md \
  "$HOME/.codex/skills/review-plan/SKILL.md"
cmp ai/codex/skills/implement-plan/SKILL.md \
  "$HOME/.codex/skills/implement-plan/SKILL.md"
cmp ai/opencode/commands/create_plan.md \
  "$HOME/.config/opencode/commands/create_plan.md"
cmp ai/opencode/commands/review_plan.md \
  "$HOME/.config/opencode/commands/review_plan.md"
cmp ai/opencode/commands/implement_plan.md \
  "$HOME/.config/opencode/commands/implement_plan.md"
```

## Disposable Fixture

Run this setup in a shell whose current directory is the worktree under review.
Create a fresh fixture for each independent scenario and harness. The guarded
correction scenario follows bounded creation in the same fixture.

````bash
set -euo pipefail
export EVAL_REPO="$EVAL_ROOT/repo"
export EVAL_OUTPUT="$EVAL_ROOT/traces"
mkdir -p "$EVAL_REPO/bin" "$EVAL_REPO/cmd/eval" "$EVAL_REPO/docs" \
  "$EVAL_REPO/internal/config" "$EVAL_REPO/internal/output" \
  "$EVAL_REPO/plans" "$EVAL_OUTPUT"
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
export CODEX_MODEL="${CODEX_MODEL:-gpt-5.3-codex-spark}"
export CODEX_SANDBOX="${CODEX_SANDBOX:-workspace-write}"
export OPENCODE_MODEL="${OPENCODE_MODEL:-opencode-go/gpt-5.6-luna}"
planner check "$EVAL_REPO/plans/review.md" --json-errors
planner check "$EVAL_REPO/plans/implement.md" --json-errors
````

The EXIT trap removes the evaluation-owned root on successful exit. Failed
setup or model execution preserves it for inspection and prints only its path.

## Invocation

Define helpers once per fixture. Codex uses natural prompts because its skills
are prompt-triggered. OpenCode uses the command named by each scenario.
The defaults use lower-tier Codex Spark and OpenCode Luna models. Scenarios
marked below override OpenCode with `opencode-go/qwen3.6-plus` to represent
local-model behavior; confirm it appears in `env OPENCODE_API_KEY="${ORIGINAL_PROVIDER_VALUES[OPENCODE_API_KEY]}" opencode models` or record that
variant as unsupported.

```bash
run_codex() {
  SCENARIO=$1
  CODEX_REQUEST=$2
  env OPENAI_API_KEY="${ORIGINAL_PROVIDER_VALUES[OPENAI_API_KEY]}" \
    codex exec --ephemeral --json --sandbox "$CODEX_SANDBOX" -m "$CODEX_MODEL" \
    -C "$EVAL_REPO" "$CODEX_REQUEST" \
    > "$EVAL_OUTPUT/$SCENARIO-codex.jsonl"
}

run_opencode() {
  SCENARIO=$1
  OPENCODE_COMMAND=$2
  OPENCODE_ARGUMENTS=${3-}
  if [ "$#" -eq 2 ]; then
    env OPENCODE_API_KEY="${ORIGINAL_PROVIDER_VALUES[OPENCODE_API_KEY]}" \
      opencode run --pure --dir "$EVAL_REPO" --model "$OPENCODE_MODEL" \
      --command "$OPENCODE_COMMAND" \
      --format json > "$EVAL_OUTPUT/$SCENARIO-opencode.jsonl"
  else
    env OPENCODE_API_KEY="${ORIGINAL_PROVIDER_VALUES[OPENCODE_API_KEY]}" \
      opencode run --pure --dir "$EVAL_REPO" --model "$OPENCODE_MODEL" \
      --command "$OPENCODE_COMMAND" \
      --format json -- "$OPENCODE_ARGUMENTS" \
      > "$EVAL_OUTPUT/$SCENARIO-opencode.jsonl"
  fi
}
```

Do not disable host sandboxing or automatic approvals for these checks. If a
harness cannot write to the disposable fixture under its normal sandbox, record
that scenario as unsupported or run it in a disposable container or VM.
`--pure` keeps unrelated OpenCode plugins from changing the eval behavior.

Record each harness version, model, request, fixture commit and status, trace
path, generated plan, Planner result, Surveil use, delegated agents, and
unexpected behavior. Keep raw traces local.

Paired commands show harness alternatives. Run each harness in its own fresh
fixture rather than running both commands against one output path.

## Scenarios

### Missing Task

```bash
run_opencode missing-task create_plan
run_codex missing-task \
  'Use create-plan, but no planning task or destination was provided.'
```

Expected behavior:

- Asks for the task, constraints, and related references.
- Does not run Surveil, Planner, or a research agent.

### Bounded Creation

```bash
OPENCODE_MODEL=opencode-go/qwen3.6-plus \
  run_opencode bounded-create create_plan \
  'Plan only changing the README heading to # Evaluated Plan. Write plans/bounded.md.'
run_codex bounded-create \
  'Use create-plan to plan only changing the README heading to # Evaluated Plan. Write plans/bounded.md.'
```

Expected behavior:

- Reads the README directly without Surveil or delegated research.
- Reserves the destination with `planner new`.
- Proposes only the README change and relevant verification.
- Produces complete, applicable diffs without placeholders.
- Passes `planner check plans/bounded.md --json-errors`.

### Occupied Destination

```bash
run_opencode occupied-destination create_plan \
  'Plan the README heading change at plans/occupied.md. Do not use another path.'
run_codex occupied-destination \
  'Use create-plan for the README heading change at plans/occupied.md. Do not use another path.'
```

Expected behavior:

- `planner new` reports that the destination exists.
- Leaves `plans/occupied.md` byte-for-byte unchanged.
- Stops instead of replacing or redirecting the plan.

### Multiple Skills

```bash
run_opencode multiple-skills create_plan \
  'Plan changing cmd/eval/main.go to print evaluated and replacing the skipped test with a behavior assertion. Write plans/go-change.md.'
run_codex multiple-skills \
  'Use create-plan to plan changing cmd/eval/main.go to print evaluated and replacing the skipped test with a behavior assertion. Write plans/go-change.md.'
```

Expected behavior:

- Loads `go-development`, `behavior-focused-testing`, and `code-documentation`
  before broader repository research.
- Names all three skills in any delegation.
- Produces complete implementation and test diffs.

### Late Skill Discovery

```bash
run_opencode late-skill create_plan \
  'Plan the requirement in docs/late-change.md without assuming its contents. Write plans/late-change.md.'
run_codex late-skill \
  'Use create-plan for the requirement in docs/late-change.md without assuming its contents. Write plans/late-change.md.'
```

Expected behavior:

- Reads the document before the late skill check.
- Loads Go, testing, and code-documentation skills before affected
  implementation decisions.
- Revisits any affected decision made before those skills loaded.

### Guarded Correction

Run bounded creation first, then use a fresh session in the same fixture:

```bash
run_opencode guarded-correction create_plan \
  'Revise the existing README diff in plans/bounded.md to use # Reviewed Plan. Preserve every unrelated section.'
run_codex guarded-correction \
  'Use create-plan to revise the existing README diff in plans/bounded.md to use # Reviewed Plan. Preserve every unrelated section.'
```

Expected behavior:

- Runs `planner inspect` immediately before correction.
- Uses one guarded `planner patch` `Update Diff` with the current token.
- Does not directly edit the existing fenced diff.
- Preserves unrelated sections and passes Planner validation again.

### Complex Research

```bash
run_opencode complex-research create_plan \
  'Plan a --format flag whose precedence spans cmd/eval, internal/config, and internal/output. The ownership boundary is uncertain; resolve it and write plans/complex.md.'
run_codex complex-research \
  'Use create-plan to plan a --format flag whose precedence spans cmd/eval, internal/config, and internal/output. The ownership boundary is uncertain; resolve it and write plans/complex.md.'
```

Expected behavior:

- Uses targeted direct inspection plus Surveil or focused delegated research to
  resolve the stated cross-cutting uncertainty.
- Verifies surprising findings directly and stops researching once ownership,
  integration, and verification decisions are supported.
- Does not expand into unrelated CLI or configuration cleanup.

### Quality Review

```bash
QUALITY_REVIEW_HEAD="$(git -C "$EVAL_REPO" rev-parse HEAD)"
OPENCODE_MODEL=opencode-go/qwen3.6-plus \
  run_opencode quality-review review_plan 'plans/review.md'
CODEX_SANDBOX=read-only run_codex quality-review \
  'Use review-plan to review plans/review.md for implementation readiness.'
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

Run complex research first, then review its result in a fresh session:

```bash
git -C "$EVAL_REPO" add plans/complex.md
git -C "$EVAL_REPO" commit -qm 'Add complex plan fixture'
BROAD_REVIEW_HEAD="$(git -C "$EVAL_REPO" rev-parse HEAD)"
run_opencode broad-review review_plan \
  'plans/complex.md. Treat configuration precedence as a high-risk integration boundary.'
CODEX_SANDBOX=read-only run_codex broad-review \
  'Use review-plan on plans/complex.md. Treat configuration precedence as a high-risk integration boundary.'
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

The request explicitly authorizes Vibe's local managed commits but no remote
actions. The two plan steps exercise a new key followed by reuse of that key.

```bash
run_opencode targeted-implementation implement_plan \
  "plans/implement.md. You may use Vibe key $EVAL_KEY and authorize its local managed commits. Do not push, open a pull request, or merge."
run_codex targeted-implementation \
  "Use implement-plan on plans/implement.md. You may use Vibe key $EVAL_KEY and authorize its local managed commits. Do not push, open a pull request, or merge."
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

Use a fresh fixture for each mode and harness. The fixture's Vibe stub returns
persisted states without launching a provider.

```bash
PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=agent-failed \
  run_opencode failed-reuse implement_plan \
  "Implement plans/implement.md with authorized Vibe key $EVAL_KEY. Do not recover prior failures."
PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=agent-failed \
  run_codex failed-reuse \
  "Use implement-plan on plans/implement.md with authorized Vibe key $EVAL_KEY. Do not recover prior failures."

PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=persistence-error \
  run_opencode persistence-reuse implement_plan \
  "Implement plans/implement.md with authorized Vibe key $EVAL_KEY."
PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=persistence-error \
  run_codex persistence-reuse \
  "Use implement-plan on plans/implement.md with authorized Vibe key $EVAL_KEY."

PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=active \
  run_opencode active-reuse implement_plan \
  "Implement plans/implement.md with authorized Vibe key $EVAL_KEY."
PATH="$EVAL_REPO/bin:$PATH" VIBE_EVAL_MODE=active \
  run_codex active-reuse \
  "Use implement-plan on plans/implement.md with authorized Vibe key $EVAL_KEY."
```

Expected behavior:

- Parses status JSON without shell evaluation and confirms its slug.
- Stops before `vibe run` for the prior `agent_failed`, non-empty
  `persistence_error`, and active state.
- Requires an explicit recovery decision even though the stub reports a commit
  for the failed run.
- Leaves `$EVAL_OUTPUT/vibe-calls.log` without a non-help `run` invocation.

### Vibe Authorization Boundary

```bash
PATH="$EVAL_REPO/bin:$PATH" run_opencode no-vibe-authorization implement_plan \
  'Implement plans/implement.md. Vibe-managed commits are not authorized.'
PATH="$EVAL_REPO/bin:$PATH" run_codex no-vibe-authorization \
  'Use implement-plan on plans/implement.md. Vibe-managed commits are not authorized.'
```

Expected behavior:

- Does not invoke `vibe run` or create a Vibe-managed branch or worktree.
- Implements directly only if the selected checkout and requested commit
  boundary are safe; otherwise asks for authorization or stops.

### Skill Routing

Run the focused cases in `skill-routing.md` for both harnesses.

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
