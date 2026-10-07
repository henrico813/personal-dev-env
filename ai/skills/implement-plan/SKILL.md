---
name: implement-plan
description: 'Implement an approved plan step by step with freshness checks and verification, for requests like "implement this plan" or "execute the approved plan"; for authoring use create-plan, for review use review-plan, and for teardown use cleanup-plan.'
metadata:
  pde-workflow: "true"
---

# Implement Plan

Implement an approved implementation plan without redesigning it during
execution.

## When to use this skill

Use this skill to implement an approved plan without redesigning it. Do not use it for authoring or reviewing the plan, or for tearing down its worktree; use create-plan for authoring, review-plan for review, and cleanup-plan for teardown instead.

Delegated prompts must not list or load workflow-orchestration skills: `create-plan`, `review-plan`, `implement-plan`, `cleanup-plan`, `design-doc`, and `research-codebase`.

## Plan Reference

Take the task from the user's request or command arguments.

If no plan reference is provided, ask for one and stop.

## Getting Started

- Read the complete plan, existing checkmarks, original requirements, and other
  directly referenced context.
- Resolve the plan using the loaded shared Planning Docs guidance.
- Identify affected domains and load matching skills before reading broader
  source, tests, or configuration.
- Keep the exact ordered skill names for any delegated execution prompt.
- Read every file named by the current step.
- Create a todo list and identify the first incomplete implementation step.
- Use Vibe only when the user explicitly authorizes its managed snapshot, step,
  progress, cleanup, and failed-run commits. When authorized, Vibe solely owns
  branch and worktree setup; otherwise follow the plan's checkout instructions
  and create commits only when separately requested.
- Pushing, opening a pull request, or merging always requires explicit user
  instruction.

Before writing commit or PR text, load `git-messages`; before using
`gh pr`, load `pull-request`.

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
  of at most 48 characters matching `[a-z0-9]+(?:-[a-z0-9]+)*`. Run
  `vibe status --key "$KEY" --long`; treat only its specific no-record error as
  absence. Also inspect Git's worktree list,
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
- Reuse prior evidence as context, not proof of freshness.
- Expand inspection only when a mismatch changes the implementation surface or
  invalidates an accepted assumption.
- Use a read-only research subagent only when a mismatch or uncovered
  dependency cannot be resolved efficiently through direct inspection.

If new evidence requires a design change, stop the affected step, explain the
mismatch, and get the proposal corrected before expanding scope.

## Execution

Run one incomplete implementation step at a time. Pass the exact current step
as the execution prompt, add an `Applicable skills:` line with every loaded
skill needed for the step, and require the worker to load available listed
skills before editing and report required skills that are unavailable.

Before delegating through Vibe, verify every required worker skill is readable
at `~/.agents/skills/<name>/SKILL.md`. If one is unavailable, do not delegate;
execute in the parent harness when safe or stop and report the missing
requirement. When a prompt file is required:

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

- `completed`: for a new key confirm `pre_run_commit` equals `$BASE`, confirm
  required skill-read evidence when the harness exposes it, inspect the commit
  and full diff, run verification, update the plan, and continue.
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
- changed source and tests satisfy `code-documentation`: existing explanations
  remain accurate, useful missing context is present, and no filler was added

Remove only drift introduced by the run, verify it, and create a separate
cleanup commit before continuing. The managed worktree must be clean before the
next Vibe run.

## Plan Updates

- Update goals and verification checkboxes as work completes; do not add
  checkboxes to implementation steps.
- Direct range-targeted Markdown edits are allowed when they preserve
  wrapped-issue frontmatter and unrelated reviewed sections.
- For guarded commands, pass the commit on the `Base commit:` line of Current
  State as `--base-commit`. The base commit is the original code version the
  plan's changes are written against. Never use a worktree's current HEAD,
  because Vibe commits after each step. If the line is missing, ask the user for
  the base commit and pass it as `--base-commit` to guarded commands and to
  `planner check`.
- If an approved correction changes an existing fenced code diff, run
  `planner inspect <plan.md> --target '<selector>' --repo <repo>
  --base-commit <commit> --code-out <new-scratch-file>`, edit the ordinary
  scratch source, then run `planner patch <plan.md> --target '<selector>'
  --expect <edit_expect> --after-file <scratch-file> --repo <repo>
  --base-commit <commit>`. Never edit that diff directly or use an unguarded
  command. Patch checks from the base commit through the edited change only,
  and later changes are checked by `planner check`.
- For coupled edits, after changing an earlier step, run `planner check
  <plan.md> --repo <repo> --json-errors` and fix each later change it reports,
  in order.
- Inspect the complete before/after plan diff after every update.
- Run `planner check <plan.md> --repo <repo> --json-errors` after every code
  change. It starts with the base commit named by the `Base commit:` line and
  tries every planned change in order. It is the only whole-plan readiness check
  and does not run tests. Do not report the plan ready until it passes.
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
- Review the final cumulative diff for scope, correctness, and source
  documentation quality.
- Reopen documentation approved in an earlier step only when later work made it
  stale, contradictory, or unsupported; do not rewrite it only for style.
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
