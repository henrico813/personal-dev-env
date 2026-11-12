# Plan Workflow Checks

Run these manual checks in fresh OpenCode and Vibe sessions after changing a
planning workflow. Use one configured model and version per run.
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
ORIGINAL_EVAL_ROOT=${EVAL_ROOT-}
ORIGINAL_EVAL_ROOT_SET=${EVAL_ROOT+x}
ORIGINAL_EVAL_HOME=${EVAL_HOME-}
ORIGINAL_EVAL_HOME_SET=${EVAL_HOME+x}
ORIGINAL_EVAL_REPO=${EVAL_REPO-}
ORIGINAL_EVAL_REPO_SET=${EVAL_REPO+x}
ORIGINAL_EVAL_OUTPUT=${EVAL_OUTPUT-}
ORIGINAL_EVAL_OUTPUT_SET=${EVAL_OUTPUT+x}
ORIGINAL_INSTALLER=${INSTALLER-}
ORIGINAL_INSTALLER_SET=${INSTALLER+x}
ORIGINAL_PDE_REPO_ROOT=${PDE_REPO_ROOT-}
ORIGINAL_PDE_REPO_ROOT_SET=${PDE_REPO_ROOT+x}
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
if [[ -z "${ORIGINAL_PROVIDER_VALUES[OPENCODE_API_KEY]}" ]]; then
  printf '%s\n' 'Set OPENCODE_API_KEY before running the evaluation' >&2
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
  if [[ -n "$ORIGINAL_EVAL_ROOT_SET" ]]; then export EVAL_ROOT="$ORIGINAL_EVAL_ROOT"; else unset EVAL_ROOT; fi
  if [[ -n "$ORIGINAL_EVAL_HOME_SET" ]]; then export EVAL_HOME="$ORIGINAL_EVAL_HOME"; else unset EVAL_HOME; fi
  if [[ -n "$ORIGINAL_EVAL_REPO_SET" ]]; then export EVAL_REPO="$ORIGINAL_EVAL_REPO"; else unset EVAL_REPO; fi
  if [[ -n "$ORIGINAL_EVAL_OUTPUT_SET" ]]; then export EVAL_OUTPUT="$ORIGINAL_EVAL_OUTPUT"; else unset EVAL_OUTPUT; fi
  if [[ -n "$ORIGINAL_INSTALLER_SET" ]]; then INSTALLER="$ORIGINAL_INSTALLER"; else unset INSTALLER; fi
  if [[ -n "$ORIGINAL_PDE_REPO_ROOT_SET" ]]; then export PDE_REPO_ROOT="$ORIGINAL_PDE_REPO_ROOT"; else unset PDE_REPO_ROOT; fi
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
cmp ai/skills/create-plan/SKILL.md \
  "$HOME/.agents/skills/create-plan/SKILL.md"
cmp ai/skills/review-plan/SKILL.md \
  "$HOME/.agents/skills/review-plan/SKILL.md"
cmp ai/skills/implement-plan/SKILL.md \
  "$HOME/.agents/skills/implement-plan/SKILL.md"
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
mkdir -p "$EVAL_REPO/cmd/eval" "$EVAL_REPO/docs" \
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

### 2. Add formatter test

Add a test for the formatter.

`internal/readme/formatter_test.go`
> Assert that the formatter ignores an environment variable.

```diff
diff --git a/internal/readme/formatter_test.go b/internal/readme/formatter_test.go
new file mode 100644
--- /dev/null
+++ b/internal/readme/formatter_test.go
@@ -0,0 +1,11 @@
+package readme
+
+import "testing"
+
+func TestHeadingDoesNotReadEnvironment(t *testing.T) {
+    t.Setenv("README_HEADING", "unexpected")
+    formatter := headingFormatter{}
+    if formatter.Heading() != "TODO" {
+        t.Fatal("heading changed from environment")
+    }
+}
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
export EVAL_MODEL="${EVAL_MODEL:-goog/qwen3.8}"
planner check "$EVAL_REPO/plans/review.md" --repo "$EVAL_REPO" \
  --base-commit "$EVAL_FIXTURE_COMMIT" --json-errors
planner check "$EVAL_REPO/plans/implement.md" --repo "$EVAL_REPO" \
  --base-commit "$EVAL_FIXTURE_COMMIT" --json-errors
````

The EXIT trap removes the evaluation-owned root on successful exit. Failed
setup or model execution preserves it for inspection and prints only its path.

## Invocation

Define the OpenCode helper once per fixture. Run it from the fixture directory so
relative paths resolve there, pass slash commands as the message, and use the
same EVAL_MODEL for every run. Vibe invokes Pi with that same model selection.

```bash
run_opencode() {
  local scenario=$1 command=$2 arguments=${3-}
  local trace_dir="$EVAL_OUTPUT/$scenario/opencode"
  mkdir -p "$trace_dir"
  opencode --version > "$trace_dir/version.txt" 2>&1
  printf '%s\n' "$EVAL_MODEL" > "$trace_dir/model.txt"
  printf '%s\n' "$command" > "$trace_dir/command.txt"
  printf '%s\n' "$arguments" > "$trace_dir/request.txt"
  local message="/$command"
  if [ -n "$arguments" ]; then message+=" $arguments"; fi
  printf '%q ' opencode run --standalone --model "$EVAL_MODEL" --format json "$message" > "$trace_dir/arguments.txt"
  printf '\n' >> "$trace_dir/arguments.txt"
  git -C "$EVAL_REPO" rev-parse HEAD > "$trace_dir/fixture-commit.txt"
  git -C "$EVAL_REPO" status --short > "$trace_dir/fixture-status.txt"
  set +e
  (
    cd "$EVAL_REPO"
    env OPENCODE_API_KEY="${ORIGINAL_PROVIDER_VALUES[OPENCODE_API_KEY]}" \
      opencode run --standalone --model "$EVAL_MODEL" --format json "$message"
  ) > "$trace_dir/stdout.jsonl" 2> "$trace_dir/stderr.txt"
  local status=$?
  set -e
  printf '%s\n' "$status" > "$trace_dir/exit-status.txt"
  git -C "$EVAL_REPO" status --short > "$trace_dir/fixture-status-after.txt"
  return "$status"
}
```

Do not disable host sandboxing or automatic approvals for these checks. If a
harness cannot write to the disposable fixture under its normal sandbox, record
that scenario as unsupported or run it in a disposable container or VM. The
current CLI has no plugin-disabling flag; use `--standalone` and the isolated
fixture directory instead.

Record each harness version, model, request, fixture commit and status, trace
path, generated plan, Planner result, delegated agents, and unexpected behavior.
Keep raw traces local.

## Scenarios

### Missing Task

```bash
run_opencode missing-task create_plan
```

Expected behavior:

- Asks for the task, constraints, and related references.
- Does not run Planner or a research agent.

### Bounded Creation

```bash
  run_opencode bounded-create create_plan \
  'Plan only changing the README heading to # Evaluated Plan. Write plans/bounded.md.'
```

Expected behavior:

- Reads the README directly without delegated research.
- Reserves the destination with `planner new`.
- Starts Current State with `Base commit: $EVAL_FIXTURE_COMMIT`. This names
  the base commit the plan describes. Check it with
  `grep -qx "Base commit: $EVAL_FIXTURE_COMMIT"
  "$EVAL_REPO/plans/bounded.md"`.
- Proposes only the README change and relevant verification.
- Produces complete, applicable diffs without placeholders.
- Passes `planner check plans/bounded.md --repo "$EVAL_REPO" --json-errors`.

### Occupied Destination

```bash
run_opencode occupied-destination create_plan \
  'Plan the README heading change at plans/occupied.md. Do not use another path.'
```

Expected behavior:

- `planner new` reports that the destination exists.
- Leaves `plans/occupied.md` byte-for-byte unchanged.
- Stops instead of replacing or redirecting the plan.

### Multiple Skills

```bash
run_opencode multiple-skills create_plan \
  'Plan changing cmd/eval/main.go to print evaluated and replacing the skipped test with a behavior assertion. Write plans/go-change.md.'
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
```

Expected behavior:

- Reads the document before the late skill check.
- Loads Go, testing, and code-documentation skills before affected
  implementation decisions.
- Revisits any affected decision made before those skills loaded.

### Guarded Correction

Run bounded creation first. Then commit the planned README change, as an
implementation run would, so HEAD no longer names the base commit.
A correction that uses HEAD as `--base-commit` fails because its README change
no longer fits the base commit.
Use a fresh session in the same fixture:

```bash
prepare_guarded_correction() {
  local harness=$1
  if [ "$(git -C "$EVAL_REPO" rev-parse HEAD)" = "$EVAL_FIXTURE_COMMIT" ]; then
    printf '%s\n' '# Evaluated Plan' > "$EVAL_REPO/README.md"
    git -C "$EVAL_REPO" commit -qm "Implement bounded plan" -- README.md
  fi
  test "$(git -C "$EVAL_REPO" rev-parse HEAD)" != "$EVAL_FIXTURE_COMMIT"
  mkdir -p "$EVAL_OUTPUT/guarded-correction/$harness"
  cp "$EVAL_REPO/plans/bounded.md" \
    "$EVAL_OUTPUT/guarded-correction/$harness/plan-before.md"
}
prepare_guarded_correction opencode
run_opencode guarded-correction create_plan \
  'Revise the existing README diff in plans/bounded.md to use # Reviewed Plan. Preserve every unrelated section.'
```

After each run, list the bases the agent passed and compare the plan:

```bash
for trace_dir in "$EVAL_OUTPUT"/guarded-correction/*/; do
  grep -o -- '--base-commit [0-9a-f]\{7,40\}' "$trace_dir/stdout.jsonl" \
    | sort -u
  diff "$trace_dir/plan-before.md" "$EVAL_REPO/plans/bounded.md" || true
done
```

Expected behavior:

- Passes `--base-commit $EVAL_FIXTURE_COMMIT` from the `Base commit:` line to
  every guarded command, never the new HEAD. The commit identifies the base
  commit the plan describes. Record a base passed through a shell variable as
  unclear.
- Runs targeted `planner inspect --target ... --repo ... --base-commit ...
  --code-out ...` immediately before the correction.
- Edits the ordinary scratch source and runs `planner patch --target ...
  --expect ... --after-file ... --repo ... --base-commit ...`.
- Does not directly edit the existing fenced diff.
- Preserves unrelated sections and passes repository-aware `planner check
  plans/bounded.md --repo "$EVAL_REPO" --json-errors` again.

### Complex Research

```bash
run_opencode complex-research create_plan \
  'Plan a --format flag whose precedence spans cmd/eval, internal/config, and internal/output. The ownership boundary is uncertain; resolve it and write plans/complex.md.'
```

Expected behavior:

- Uses targeted direct inspection plus focused delegated research to
  resolve the stated cross-cutting uncertainty.
- Verifies surprising findings directly and stops researching once ownership,
  integration, and verification decisions are supported.
- Does not expand into unrelated CLI or configuration cleanup.

### Quality Review

```bash
QUALITY_REVIEW_HEAD="$(git -C "$EVAL_REPO" rev-parse HEAD)"
  run_opencode quality-review review_plan 'plans/review.md'
test "$(git -C "$EVAL_REPO" rev-parse HEAD)" = "$QUALITY_REVIEW_HEAD"
test -z "$(git -C "$EVAL_REPO" status --porcelain)"
```

Expected behavior:

- Invokes review rather than plan creation.
- Performs the complete direct quality review without automatically launching
  three reviewers.
- Marks the speculative abstraction, placeholder, verification gap, and
  `TestHeadingDoesNotReadEnvironment` as required corrections.
- Does not request other low-value tests.
- Separates optional suggestions and identifies affected plan sections.

### Broad Review

Run complex research first, then review its result in a fresh session:

```bash
git -C "$EVAL_REPO" add plans/complex.md
git -C "$EVAL_REPO" commit -qm 'Add complex plan fixture'
BROAD_REVIEW_HEAD="$(git -C "$EVAL_REPO" rev-parse HEAD)"
run_opencode broad-review review_plan \
  'plans/complex.md. Treat configuration precedence as a high-risk integration boundary.'
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

The request allows the step commits but no remote actions. The
fixture plan has no `Base commit:` line, so add one first. The two
plan steps check that the second step commits on top of the first.

```bash
sed -i "/^### Current State$/a\\
\\
Base commit: $EVAL_FIXTURE_COMMIT" "$EVAL_REPO/plans/implement.md"
git -C "$EVAL_REPO" commit -qm "Add base commit" -- plans/implement.md
run_opencode targeted-implementation implement_plan \
  "plans/implement.md. Do not push, open a pull request, or merge."
```

After the run, find the build's worktree and read its commits:

```bash
WORKTREE=$(git -C "$EVAL_REPO" worktree list --porcelain |
  awk -v repo="$EVAL_REPO" 'sub(/^worktree /, "") && $0 != repo')
git -C "$WORKTREE" log --oneline "$EVAL_FIXTURE_COMMIT..HEAD"
git -C "$WORKTREE" diff --stat "$EVAL_FIXTURE_COMMIT"
git -C "$WORKTREE" status --short
```

Expected behavior:

- Follows implement-plan's `references/build.md`. Before the first
  step, runs the Freshness Check and `planner check` on the plan, and
  stops on any error.
- Creates one worktree with `--base $EVAL_FIXTURE_COMMIT` and one run
  folder with `mktemp -d`.
- Writes `MSG1` and `MSG2` in the run folder by the `git-messages`
  skill, and runs `msg fmt` and `msg lint` on each before step 1.
- Runs `build-step.sh` once per step through `step<n>.sh`. When the
  session's model is the local model, the session runs `step<n>.sh`
  itself instead of starting a job.
- Never edits worktree files by hand.
- Each `job<n>/summary.txt` starts with `result: pass` and ends with
  `done`.
- The log shows two commits, one per step. The diff changes only
  `README.md` and `cmd/eval/main.go`, and the worktree is clean.
- Applies the existing completion gate to source and test
  documentation without adding a separate review pass.
- Runs the complete verification suite, reviews the cumulative diff,
  and updates final plan status before reporting completion.
- Ends with the report lines from `build.md`, with no `Escalated`
  line, and does not perform a remote action.

### Skill Routing

Run the focused cases in `skill-routing.md` with the configured model.

## Acceptance

For every generated or revised plan, verify:

- Every intended file and line is represented.
- Proposed hunks match current fixture source.
- Diffs contain no placeholders or omitted code.
- No `<optional: ...; delete if unused>` notes placeholder from
  `planner new` is left; planner check does not catch it.
- Prose paragraphs and list items are one line each, not hand-wrapped.
- Unrelated files and cleanup are excluded.
- Targeted corrections preserve accepted sections and untouched fenced diffs.
- Automated verification is runnable and relevant.
- Review-only scenarios leave repository status unchanged.

If a harness does not expose a required event, record it as unsupported rather
than passed. Delete only exact fixture, trace, and Vibe paths recorded
during the evaluation.
