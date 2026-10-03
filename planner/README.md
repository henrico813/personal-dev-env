# Planner

Planner is the planning CLI in this repository. It creates and revises plan
documents whose code changes are written as unified diffs. Before it writes or
approves a change, Planner checks whether that change fits the code version the
plan was written for.

## The base commit

A plan records the full Git commit ID for its base commit. The base commit is
the original code version the plan's changes are written against, meaning the
files as they looked when the plan was created. Planner needs that starting
point to tell whether each planned change still fits.

Create-plan stores the base commit in the first line of `### Current State`:

    Base commit: 4f9c...<full 40-character or 64-character SHA>

`planner check` reads that line. `planner inspect` and `planner patch` still
require the same commit as `--base-commit`. No command defaults to `HEAD`.

## Why guarded commands need the base commit

Each planned change describes how to edit a file as it existed in the base
commit. `planner inspect`, `planner patch`, and `planner check` create a
temporary copy of that version, then try the planned changes on it.

`--base-commit` must be a full commit ID. `HEAD` is rejected, and so are branch
names and tags. The reason is not authentication; it is that the same bytes have
to be found later, even after the branch has moved.

If you pass the current `HEAD` instead of the base commit:

- diffs for earlier steps no longer apply, because those changes are already in
  `HEAD`;
- a diff can apply against the wrong content and verify the wrong source;
- `check` can report readiness for a plan that does not match the tree it will
  actually be applied to.

## A/B/C example: why checks start at A

Plan creation reads the source at commit A and records A as the base commit.
Each implementation step is then committed by Vibe, moving `HEAD` forward:

    A  plan created; changes describe files at A
    B  Vibe commits step 1
    C  Vibe commits step 2        <- HEAD is now C

The plan's changes still describe A to B and B to C. Planner must therefore
start at A and try each change in order:

    base A -> apply step 1 -> apply step 2

If a guarded command starts at C instead, step 1's diff expects the file as it
was at A, but the file at C already contains step 1's changes. The apply fails,
or succeeds against content that only happens to match. Pass A, not C.

## How Planner uses the base commit

Planner uses the base commit in three places:

- `planner inspect` starts from the base commit, tries every earlier planned
  change, and exports the source just before or after the selected change. The
  returned `edit_expect` token is tied to the plan bytes, selector, and base
  commit.
- `planner patch` checks that token, starts from the base commit, and tries
  every change through the edited one before writing the plan. It does not try
  later changes.
- `planner check` is the final plan check: it reports every structure violation,
  then starts from the base commit and tries every planned change in order. It
  does not run tests or check behavior.

The disposable repository shares read-only Git objects with your repository and
is removed afterwards. Planner never stages, stashes, resets, or commits your
worktree, and uncommitted or untracked files are not part of the base commit.

## What the base commit does not protect against

The commit ID identifies a starting point for checking planned changes. It does
not:

- prove the commit is trusted, signed, or safe, and it is not an approval step;
- lock the plan to a branch, remote, or author;
- include uncommitted or untracked source;
- guarantee the applied result still compiles or passes tests (`check` reports
  `behavior_checked: false`);
- stop someone from passing a different valid commit ID for the same plan.

It makes the check repeatable. It does not make the plan authorized or correct.

## Who this matters to

For ordinary users who run Planner through the create-plan and implement-plan
prompts, the base commit is handled by the prompt and mostly invisible. The
prompt should record it once and reuse it.

It matters directly to maintainers of those prompts and to anyone writing or
running plan evaluations. An eval that creates a plan, commits an implementation
step, and then revises the plan will catch a workflow that used `HEAD`: the edit
fails because the diff no longer applies. When checking such a run, confirm that
every guarded command used the base commit, not the new `HEAD`.

## Issue frontmatter

`planner new` writes a plain scaffold by default. When the plan belongs in the
PDE vault, pass `--issue --project <name>` to prepend the vault issue
frontmatter. `--project` is required with `--issue` and rejected without it,
`date_created` is today's local date, and `--diff`/`--dry-run` preview the same
bytes that would be written. The block matches what `planner check` accepts, so
a wrapped plan keeps parsing without further edits.
