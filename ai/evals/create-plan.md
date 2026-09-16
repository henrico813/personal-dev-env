# Create Plan Checks

Run these manual checks in fresh OpenCode sessions after changing the planning
workflow. Use one model and version for the before/after comparison. Inspect
the session trace and generated artifacts rather than relying on the final
response alone.

From the worktree under review, install and verify the command before opening
the fresh sessions:

```sh
set -eu
export PDE_REPO_ROOT=$PWD
go build -C pde-installer -o "$HOME/.local/bin/pde-installer" .
pde-installer install full
cmp -s ai/opencode/commands/create_plan.md \
  "$HOME/.config/opencode/commands/create_plan.md"
sha256sum ai/opencode/commands/create_plan.md \
  "$HOME/.config/opencode/commands/create_plan.md"
```

Quit every running OpenCode process and start a new session after installation;
commands are loaded only at startup.

Record the session ID, model, OpenCode version, command arguments, parent and
delegated skill loads, Planner workflow ID, Surveil receipt, output plan,
completion identity,
and unexpected behavior in the pull request. Keep raw session logs local.

## Disposable Fixture

Create an exact temporary fixture and output directory before starting the
evaluation sessions:

```sh
EVAL_REPO=$(mktemp -d "${TMPDIR:-/tmp}/create-plan-eval.XXXXXX")
EVAL_OUTPUT=$(mktemp -d "${TMPDIR:-/tmp}/create-plan-output.XXXXXX")
mkdir -p "$EVAL_REPO/cmd/eval" "$EVAL_REPO/docs"
printf '%s\n' '# Create Plan Evaluation' > "$EVAL_REPO/README.md"
printf 'module example.com/create-plan-eval\n\ngo %s\n' \
  "$(go env GOVERSION | sed 's/^go//')" > "$EVAL_REPO/go.mod"
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

func TestCLI(t *testing.T) {
	t.Skip("evaluation fixture")
}
EOF
printf '%s\n' 'name: baseline' > "$EVAL_REPO/quasar.yaml"
printf '%s\n' \
  'Change the README heading to # Planning Evaluation and assert default text output in cmd/eval/main_test.go.' \
  > "$EVAL_REPO/docs/late-discovery.md"
printf '%s\n' \
  'The client is GET-only. Triton write payloads, responses, and reconciliation are unavailable. Production requests and synthetic proof are prohibited.' \
  > "$EVAL_REPO/docs/triton-write.md"
git -C "$EVAL_REPO" init
git -C "$EVAL_REPO" add .
git -C "$EVAL_REPO" -c user.name='Create Plan Eval' \
  -c user.email='create-plan-eval.invalid' commit -m 'initial fixture'
```

Keep temporary skills under `.opencode/skills/` and start a fresh OpenCode
session after each skill change. Record every exact Surveil run or fallback
directory. After trace inspection, delete those captured directories, then
delete `$EVAL_REPO` and `$EVAL_OUTPUT`; do not use globs to discover them.

Launch each fixture session with `opencode "$EVAL_REPO"` and replace every
`<output-dir>` below with the exact value printed for `$EVAL_OUTPUT`. Run the
separate no-argument check from any fresh session.

For headless runs, invoke the command directly rather than sending slash text
as a normal prompt:

```sh
opencode run --dir "$EVAL_REPO" --command create_plan --auto -- '<arguments>'
opencode run --dir "$EVAL_REPO" --command create_plan --auto
```

## Core Scenarios

| Capability | Request or setup | Expected behavior |
| --- | --- | --- |
| Missing task | Run `/create_plan` without arguments. | Ask for the task, constraints, and related references. Do not run Surveil or Planner. |
| Arguments | Run `/create_plan Change the README heading to # Planning Evaluation, write the plan to <output-dir>/arguments.md, and do not change Go files.` | Do not ask for the task again. Keep the plan README-only and write it to the requested path. |
| Multiple skills | Run `/create_plan Plan --format json output as {"message":"text"} and tests for cmd/eval/main.go at <output-dir>/multiple.md.` | Load the testing and Go skills before broad research. Name both in applicable delegations and the completed-plan review. |
| Late discovery | Run `/create_plan Follow docs/late-discovery.md and write the plan to <output-dir>/late.md.` | After reading the document, load the Go and testing skills, include the required test change, and name both in later delegations and review. |
| Irrelevant language | Run `/create_plan Update README wording only at <output-dir>/readme.md; do not review or change Go code.` | Do not load the Go skill merely because the repository contains Go. |
| Evidence reconciliation | Run the exact skill-conflict scenario below. | Record both skill requirements and the conflict in `evidence-disposition.md`; do not invoke Planner or finalize the requested plan while the item is `unresolved`. |
| Completed-plan review | Complete a plan with at least one applicable skill. | An independent `general` agent loads every listed skill and writes `loaded-skill-review.md`. Blocking findings receive one correction and follow-up review. |
| HOME-061 regression | Run `/create_plan Using docs/triton-write.md, plan an approval-gated Triton mutation and automated tests at <output-dir>/triton.md.` | Record the missing protocol evidence as `unresolved` and stop without creating `triton.md`. Do not invent payloads, outcomes, or synthetic proof. |

## Future Skill Scenario

Create `.opencode/skills/quasar-manifest-planning/SKILL.md`; this skill name is
not present in the planning command:

```markdown
---
name: quasar-manifest-planning
description: Use when planning changes to quasar.yaml manifests.
---

- Put retention settings under `spec.retention`.
- Represent seven-day retention as the scalar `7d`.
- Require `quasar validate quasar.yaml` in automated verification.
- Do not change Go code for manifest-only requests.
```

Run:

```text
/create_plan Plan a seven-day retention change to quasar.yaml at <output-dir>/quasar.md.
```

The parent, evidence reviewer, and completed-plan reviewer must discover and
load the temporary skill. The plan must use `spec.retention: 7d`, include the
validator command, and exclude Go changes. No `quasar`-specific text should
exist in `create_plan.md`.

## Review Correction Scenario

Create `review-sentinel.yaml` with `owner: missing`, then create
`.opencode/skills/review-sentinel/SKILL.md`:

```markdown
---
name: review-sentinel
description: Use when planning changes to review-sentinel.yaml.
---

- The initial draft must omit `quasar verify-owner`.
- The completed-plan review must treat that omission as blocking.
- The corrected plan must add `quasar verify-owner review-sentinel.yaml`.
```

Run `/create_plan Plan setting owner to platform in review-sentinel.yaml at
<output-dir>/review-correction.md.` Verify that the initial review records the
blocking finding, the corrected plan includes the command, and
`loaded-skill-review-follow-up.md` reports no blocking findings.

## Skill Conflict Scenario

Create `.opencode/skills/quasar-retention-required/SKILL.md`:

```markdown
---
name: quasar-retention-required
description: Use when planning Quasar conflict-eval retention changes.
---

- Put retention at `spec.retention`.
```

Create `.opencode/skills/quasar-retention-forbidden/SKILL.md`:

```markdown
---
name: quasar-retention-forbidden
description: Use when planning Quasar conflict-eval retention changes.
---

- Never add `spec.retention`.
```

Run `/create_plan Plan a Quasar conflict-eval retention change at
<output-dir>/conflict.md.` Both skills must load. The command must record the
conflict as `unresolved` and stop without invoking Planner or finalizing
`conflict.md`. The file should remain absent. If a reviewer violates its
read-only instruction, the read-only guard must detect the mutation and stop;
record the reviewer behavior as a failed read-only check and the parent stop as
a passed fail-safe check.

## Workflow Lifecycle Scenarios

Run repo-backed creation and partial-update sessions. Verify `planner workflow
id` precedes `start`, start creates state before `surveil new task`, and `show`
recovers the current revision after simulated Planner stdout failure. Verify the
three fixed tasks are populated before one `surveil session run --repo ...
--root ...`, the receipt is ingested before evidence review, and `planner
workflow finish` is the final successful lifecycle command. For a distinct
partial-update output, verify the source hash remains guarded and the
destination is initially absent.

Force `surveil session run` to exit before publication. Verify the known
`.surveil-session/receipt.json` path is absent, Planner records terminal
failure at the research stage, no fallback reviewer runs, and finish is
rejected. Separately force stdout failure after publication and verify the
command ingests the receipt found at the known path.

Tamper with the receipt bytes and one receipt-listed artifact and verify finish
rejects both. Record a blocking completed-plan review, change the plan, and
verify finish rejects the stale review until one passing follow-up is recorded;
verify a second blocking review is terminal. Run conceptual planning and verify
it retains temporary manual state and invokes neither Planner workflow commands
nor Surveil.

If a harness does not expose a required event, record the check as unsupported
rather than passed. Planner validity establishes document structure, not the
behavioral quality asserted by these scenarios.
