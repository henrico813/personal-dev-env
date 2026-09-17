---
description: Implement an approved plan with targeted freshness checks and verification
---

# Implement Plan

Implement an approved implementation plan without redesigning it during
execution.

## Plan Reference

$ARGUMENTS

If no plan reference is provided, ask for one and stop.

## Getting Started

- Read the complete plan, existing checkmarks, original requirements, and other
  directly referenced context.
- Resolve the plan using the loaded shared Planning Docs guidance.
- Identify affected domains and load matching skills before reading broader
  source, tests, or configuration.
- Read every file named by the current step.
- Create a todo list and identify the first incomplete implementation step.
- Use Vibe only when the user explicitly authorizes its managed snapshot, step,
  progress, cleanup, and failed-run commits. When authorized, Vibe solely owns
  branch and worktree setup; otherwise follow the plan's checkout instructions
  and create commits only when separately requested.
- Pushing, opening a pull request, or merging always requires explicit user
  instruction.

Before using Vibe, inspect its installed command shape:

```bash
which vibe
vibe --help
vibe run --help
```

Choose a `provider/model` selector before invoking Vibe and pass it unchanged
with `--model "$MODEL"`. Resolve provider-specific failures from available logs
and configured providers without exposing credentials.

## Freshness Check

The approved plan is the primary implementation context. Do not repeat the
plan-generation research workflow by default.

Before every implementation step:

- Select the exact checkout that will receive edits before checking freshness.
- For a candidate new Vibe key, resolve a clean full commit SHA and choose a key
  of at most 48 characters matching `[a-z0-9]+(?:-[a-z0-9]+)*`, beginning with
  its short SHA, so Vibe normalization does not change it. Run
  `vibe status --key "$KEY" --long`; treat only its specific
  no-record error as absence. Also inspect Git's worktree list,
  `refs/heads/vibe/$KEY`, and the expected `worktrees/$KEY` path. Use
  `--base "$BASE"` only when none exists. Vibe rejects a concurrent run or
  existing managed state atomically. Stop if relevant dirty content is not
  represented by that commit.
- For a reused Vibe key, parse `vibe status --key "$KEY" --long` as JSON without
  evaluating its values in a shell. Require `slug == $KEY`, no
  `persistence_error`, `phase == finished`, and `terminal_status` equal to
  `completed` or `noop`. Any prior failure or active/incomplete state requires
  explicit recovery. Then verify its managed worktree path, `vibe/$KEY` branch, HEAD,
  ownership, and cleanliness. Omit `--base`; Vibe rejects it for existing state.
- For non-Vibe execution, create or select the target worktree first.
- In that checkout, verify proposed hunk context, affected files, directly
  affected callers, relevant tests, and configuration.
- Reuse prior evidence as context, not proof of freshness. Do not search for old
  Surveil runs when none were supplied.
- Expand inspection only when a mismatch changes the implementation surface or
  invalidates an accepted assumption.
- Use Surveil or a read-only research subagent only when a mismatch or uncovered
  dependency cannot be resolved efficiently through direct inspection.

If new evidence requires a design change, stop the affected step, explain the
mismatch, and get the proposal corrected before expanding scope.

## Execution

Run one incomplete implementation step at a time. Pass the exact current step
as the execution prompt. When a prompt file is required:

```bash
prompt_dir="$(mktemp -d "${TMPDIR:-/tmp}/implement-plan-prompt.XXXXXX")"
```

Write the exact step to `"$prompt_dir/prompt.md"`.

For the first run of a new Vibe key:

```bash
vibe run --key "$KEY" --base "$BASE" \
  --prompt-file "$prompt_dir/prompt.md" --model "$MODEL"
```

For a reused key, omit `--base` and use the managed worktree inspected through
`vibe status`.

Vibe owns worktree creation, sandboxing, execution, and result reporting. Do not
create its branch or worktree manually.

After each run, parse the final JSON. Stop on any non-empty
`persistence_error`, regardless of status. Otherwise handle status as follows:

- `completed`: for a new key confirm `pre_run_commit` equals `$BASE`, then
  inspect the commit and full diff, run verification, update the plan, and
  continue.
- `noop`: continue only when the step was already complete or intentionally a
  no-op and verified.
- `agent_failed`: inspect artifacts, the reported commit, and the complete diff.
  Retry automatically only when no commit or worktree changes were produced;
  otherwise stop and require an explicit recovery decision.
- `commit_failed` or `refused_dirty`: stop and inspect the reported worktree.
- `snapshot_failed` or `wrapper_failed`: stop and inspect all persisted
  artifacts before deciding whether recovery is safe.
- `setup_error`: resolve provider-specific failures, otherwise stop.
- Any unknown status: stop and inspect rather than guessing.

Before the next step, confirm:

- changes match the approved step
- no unrelated files changed
- no secrets or generated artifacts were committed
- relevant verification passes

Remove only drift introduced by the run, verify it, and create a separate
cleanup commit before continuing. The managed worktree must be clean before the
next Vibe run.

## Plan Updates

- Update goals and verification checkboxes as work completes; do not add
  checkboxes to implementation steps.
- Direct range-targeted Markdown edits are allowed when they preserve
  wrapped-issue frontmatter and unrelated reviewed sections.
- If an approved correction changes an existing fenced code diff, run
  `planner inspect` immediately before a single guarded `planner patch`
  `Update Diff`. Never edit that diff directly or use an unguarded command.
- Inspect the complete before/after plan diff after every update.
- Run `planner check <plan.md> --json-errors` after every update.
- Stop if parsing fails or collateral content changed.
- Do not mark manual verification complete unless the user confirms it.
- If the plan is inside the managed worktree, include its verified progress
  update in a progress commit before the next Vibe run.

## Verification

After each phase:

- Run the plan's automated checks, preferring suitable `make` targets.
- Fix failures before proceeding.
- Review behavior in the broader codebase, not only the changed function.
- Preserve reviewed implementation decisions unless new evidence invalidates
  them.
- Perform feasible manual verification, but leave user-only checks unchecked.

Pause only when something failed, a required manual check is infeasible, the
plan no longer matches the repository, or the user requested a stop.

## Completion

Before finishing:

- Confirm all approved steps are implemented or explicitly blocked.
- Run the complete verification suite from the plan.
- Review the final cumulative diff for scope and correctness.
- Update plan status and checklists truthfully.

Summarize:

- steps and statuses
- commits, worktree, and branch
- verification results
- log and artifact paths
- remaining manual checks or risks

Before writing commit or pull request messages, load and follow the
`git-messages` skill. Do not push, create a pull request, or merge unless the
user requested it.
