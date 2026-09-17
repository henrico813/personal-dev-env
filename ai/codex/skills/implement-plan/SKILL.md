---
name: implement-plan
description: Use when the user asks to implement an approved plan with staged execution, fresh-source checks, verification, and guarded progress updates.
---

# Implement Plan

Execute an approved implementation plan step by step. Treat the plan as the
source of intended changes while validating it against the actual execution
checkout before every step.

## Invocation

Read the plan reference and any execution constraints from the user's request.
If no plan reference was provided, ask for it and stop.

## Getting Started

- Read the plan completely.
- Read original requirements and other directly referenced context, identify
  affected domains, and load matching skills.
- Read every file named in the current step after that skill gate.
- If delegating work, include the exact names of applicable loaded skills and
  require the subagent to load available skills before working.
- Use Vibe only when the user explicitly authorizes its managed snapshot, step,
  progress, cleanup, and failed-run commits. When authorized, Vibe solely owns
  branch and worktree setup; otherwise follow the plan's checkout instructions
  and create commits only when separately requested.
- Pushing, opening a pull request, or merging always requires explicit user
  instruction.

Before using Vibe, inspect its installed command shape with `which vibe`, `vibe
--help`, and `vibe run --help`. Choose a `provider/model` selector and pass it
unchanged with `--model`.

## Freshness Gate

Before each step, inspect the checkout that will actually execute the step.

- For a candidate new Vibe key, resolve a clean full commit SHA and choose a key
  of at most 48 characters matching `[a-z0-9]+(?:-[a-z0-9]+)*`, beginning with
  its short SHA, so Vibe normalization does not change it. Run
  `vibe status --key <key> --long`; treat only its specific
  no-record error as absence. Also inspect Git's worktree list,
  `refs/heads/vibe/<key>`, and the expected `worktrees/<key>` path. Use
  `--base <full-sha>` only when none exists. Vibe rejects a concurrent run or
  existing managed state atomically. Stop if relevant dirty content is not
  represented by that commit.
- For a reused Vibe key, parse `vibe status --key <key> --long` as JSON without
  evaluating its values in a shell. Require `slug == <key>`, no
  `persistence_error`, `phase == finished`, and `terminal_status` equal to
  `completed` or `noop`. Any prior failure or active/incomplete state requires
  explicit recovery. Then verify its managed worktree path, `vibe/<key>` branch, HEAD,
  ownership, and cleanliness. Omit `--base`; Vibe rejects it for existing state.
- Without Vibe, select or create the target worktree first, then inspect that
  checkout.
- Re-read the step's files, callers, related tests, and any directly affected
  config or docs.
- Compare the plan's assumptions and proposed diff with current source.
- If current source invalidates behavior, scope, or structure, stop the affected
  step and get the plan corrected through the guarded Planner workflow before
  implementing.
- Do not rely on a marker file or pre-existing worktree alone as proof of
  freshness.

## Step Execution

For each step:

1. Apply the freshness gate.
2. Implement only the current step's intended scope.
3. Follow current repository patterns where they improve correctness and
   consistency without changing the plan's intent.
4. Treat added abstractions, wrappers, compatibility code, and unrelated
   cleanup as suspect unless a present requirement or verified invariant needs
   them.
5. Run the step's verification commands.
6. Inspect the full diff against the pre-step commit, not only the latest commit
   or summary.
7. Remove unsupported drift before continuing.
8. Update the plan's progress only after code and verification agree.

For the first run of a proven-new key, invoke `vibe run` with the key, full
`--base` SHA, exact step prompt file, and selected model. For later runs, use the
same key and omit `--base`. Vibe owns managed branch and worktree creation; do
not create either manually.

## Vibe Result Handling

Inspect the JSON result explicitly. Stop on any non-empty `persistence_error`,
regardless of status. Otherwise handle status as follows:

- `completed`: for a new key confirm `pre_run_commit` equals the full base SHA,
  then inspect the commit and full diff, run verification, update the plan, and
  continue.
- `noop`: inspect the worktree and artifacts; continue only if the step was
  already satisfied and verified.
- `agent_failed`: inspect artifacts, the reported commit, and the complete diff.
  Retry automatically only when no commit or worktree changes were produced;
  otherwise stop and require an explicit recovery decision.
- `commit_failed` or `refused_dirty`: stop and inspect the reported worktree.
- `snapshot_failed` or `wrapper_failed`: stop and inspect all persisted
  artifacts before deciding whether recovery is safe.
- `setup_error`: resolve provider-specific failures, otherwise stop.
- Any unknown status: stop and inspect rather than guessing.

## Diff and Drift Review

Compare the full step diff with the plan. Classify each deviation as:

- Required for correctness or compilation.
- A justified repository-pattern adjustment.
- Incidental drift outside the step.

Remove only drift introduced by the run, verify it, and create a separate
cleanup commit before continuing. The managed worktree must be clean before the
next Vibe run.

## Guarded Plan Updates

Use `planner help` when needed.

- For ordinary prose, status, and checklist updates, write or edit Markdown
  directly with range-targeted changes.
- Before every revision to an existing fenced code diff, run
  `planner inspect <plan.md>` immediately, then use its current
  `update_diff_expect` token in a single-operation guarded `planner patch`
  `Update Diff`.
- Run `planner check <plan.md> --json-errors` after every update.
- Stop if parsing fails or collateral content changed.
- Do not mark manual verification complete unless the user confirms it.
- If the plan is inside the managed worktree, include its verified progress
  update in a progress commit before the next Vibe run.

## Completion

Before finishing:

- Confirm all approved steps are implemented or explicitly blocked.
- Run the complete verification suite from the plan.
- Review the final cumulative diff for scope and correctness.
- Update plan status and checklists truthfully.
- Summarize what changed, verification performed, justified deviations, and
  remaining blockers.

Before writing commit or pull request messages, load and follow the
`git-messages` skill. Do not push, create a pull request, or merge unless the
user requested it.
