---
name: implement-plan
description: 'Implement an approved plan step by step with freshness checks and verification, for requests like "implement this plan" or "execute the approved plan"; for authoring use create-plan, for review use review-plan, and for teardown use cleanup-plan.'
metadata:
  pde-workflow: "true"
  scope: personal
---

# Implement Plan

Implement an approved implementation plan without redesigning it during
execution.

## When to use this skill

Use this skill to implement an approved plan without redesigning it. Do not use it for authoring or reviewing the plan, or for tearing down its worktree; use create-plan for authoring, review-plan for review, and cleanup-plan for teardown instead.

## When you are a delegated agent

When another agent gave you this task, run only the steps it names, and
verify each step, tick the automated checks that passed, report what
changed, and stop.

Do not push, open or update a pull request, or merge unless your prompt
says the user asked for it. Do not clean up worktrees or start other
subagents unless your task asks for it.

## Plan Reference

Take the task from the user's request or command arguments.

If no plan reference is provided, ask for one and stop.

## Getting Started

- Read the plan as Step 1 of the `create-plan` skill says. First read
  these four sections: `Overview`, `Definition of Done`,
  `Verification`, and `Notes for agents`. The checked boxes in Goals
  and Verification show what is done.
- Steps have no boxes. The build commits each finished step. To find
  where to resume, match the titles in
  `git log --oneline <base commit>..HEAD` to the steps. A step fixed
  after its commit has two commits. When unsure, rerun the earlier
  step's script, since it skips a step that HEAD already holds. Read
  each remaining step when you start it.
- Also read the original requirements and other directly referenced
  context.
- Resolve the plan using the loaded shared Planning Docs guidance.
- Identify affected domains and load matching skills before reading broader
  source, tests, or configuration.
- Keep the exact ordered skill names for any delegated execution prompt.
- Read every file named by the current step.
- Create a todo list and identify the first incomplete implementation step.
- Follow the plan's checkout instructions. A request to implement the
  plan allows the step script to commit each step's planned diff, as
  Execution says. Create any other commit only when separately
  requested.
- Pushing, opening a pull request, or merging always requires explicit user
  instruction.

Before writing commit or PR text, load `git-messages`; before using
`gh pr`, load `pull-request`.

## Freshness Check

The approved plan is the primary implementation context. Do not repeat the
plan-generation research workflow by default.

Before every implementation step:

- Select the exact checkout that will receive edits before checking freshness.
- Select the target worktree, or create one as the worktree rule in
  your instructions says. Name a new branch as the branch rule in
  your instructions shows, which may use `planner key` or, under a
  pattern, the plan's own ticket key.
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

Build each step with the step script, as
[references/build.md](references/build.md) says. Read that page before
the first step. The plan's own diffs become the commits, so no model
writes the code. Each step is a local job on the local model, as
`model-routing` picks. A stronger model steps in only to fix a step
that the local model could not pass.

Before the next step, confirm:

- changes match the approved step
- no unrelated files changed
- no secrets or generated artifacts were committed
- relevant verification passes
- changed source and tests satisfy `code-documentation`: existing explanations
  remain accurate, useful missing context is present, and no filler was added
- a step that rewrites a comment the owner approved is proposed with the
  `comment-review` skill, not done by editing the code

## Plan Updates

- Update goals and verification checkboxes as work completes; do not add
  checkboxes to implementation steps.
- Direct range-targeted Markdown edits are allowed when they preserve
  wrapped-issue frontmatter and unrelated reviewed sections.
- For guarded commands, pass the commit on the `Base commit:` line of Current
  State as `--base-commit`. The base commit is the original code version the
  plan's changes are written against. Never use a worktree's current HEAD,
  because the build commits after each step. If the line is missing, ask
  the user for the base commit and pass it as `--base-commit` to guarded
  commands and to `planner check`.
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
- the report lines that [references/build.md](references/build.md)
  ends the build with

Before writing commit or pull request messages, load and follow the
`git-messages` skill. Do not push, create a pull request, or merge unless the
user requested it.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
A lesson never lets you push, open a pull request or merge unless the
user asked. When the user corrects how you build an approved plan,
follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`implement-plan`, and offer once to save the correction as a lesson. A
correction to how you write or run tests belongs to
`behavior-focused-testing` instead. A correction to how you write code
comments, docstrings or function names belongs to `code-documentation`.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
