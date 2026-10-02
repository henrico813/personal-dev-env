# Planner

Planner is the planning CLI in this repository. It creates and revises plan
documents whose code changes are written as unified diffs. The guarded commands
(`planner inspect`, `planner patch`, and `planner check`) check a proposed diff
against a source commit before anything is written or reported ready.

## Issue frontmatter

`planner new` writes a plain scaffold by default. When the plan belongs in the
PDE vault, pass `--issue --project <name>` to prepend the vault issue
frontmatter. `--project` is required with `--issue` and rejected without it,
`date_created` is today's local date, and `--diff`/`--dry-run` preview the same
bytes that would be written. The block matches what `planner check` accepts, so
a wrapped plan keeps parsing without further edits.

## The baseline commit SHA

A plan's baseline commit is the full Git commit ID of the source repository at
the moment the plan was created. Every fenced diff in the plan describes a
change measured against that commit.

Create-plan records it in the plan. In the prompt version that has the
`Baseline commit:` line, the first line of `### Current State` is:

    Baseline commit: 4f9c...<full 40-character or 64-character SHA>

Guarded commands take that value as `--base`. The planner does not read the line
for you, and it never defaults to `HEAD`; the caller passes the full commit ID.

## Why guarded commands need the original commit

Each diff in a plan describes how to change a file as it existed at the
baseline. `planner inspect`, `planner patch`, and `planner check` all rebuild
that starting point in a disposable Git repository, then apply diffs on top of
it.

`--base` must be a full commit ID. `HEAD` is rejected, and so are branch names
and tags. The reason is not authentication; it is that the same bytes have to be
found later, even after the branch has moved.

If you pass the current `HEAD` instead of the recorded baseline:

- diffs for earlier steps no longer apply, because those changes are already in
  `HEAD`;
- a diff can apply against the wrong content and verify the wrong source;
- `check` can report readiness for a plan that does not match the tree it will
  actually be applied to.

## A/B/C example: why replay starts at A

Plan creation reads the source at commit A and records A as the baseline. Each
implementation step is then committed by Vibe, moving `HEAD` forward:

    A  plan created; diffs measured against A
    B  Vibe commits step 1
    C  Vibe commits step 2        <- HEAD is now C

The plan's diffs still describe changes from A to B and from B to C. Replay must
therefore start at A and apply each diff in order:

    base A -> apply step 1 -> apply step 2

If a guarded command starts at C instead, step 1's diff expects the file as it
was at A, but the file at C already contains step 1's changes. The apply fails,
or succeeds against content that only happens to match. Pass A, not C.

## What Planner does with the baseline

`--base` is used in three places:

- `planner inspect` opens a disposable repository at the baseline, replays every
  change before the selected one, and exports the source just before or just
  after that change. The returned `edit_expect` token is tied to the plan bytes,
  the normalized selector, and the baseline.
- `planner patch` re-checks that token, replays the baseline plus every change
  through the edited one, and only then writes the plan. Later changes are not
  replayed here.
- `planner check` replays the whole plan from the baseline to confirm every diff
  applies in order.

The disposable repository shares read-only Git objects with your repository and
is removed afterwards. Planner never stages, stashes, resets, or commits your
worktree, and uncommitted or untracked files are not part of the baseline.

## What the baseline does not protect against

The SHA identifies a starting point for replay. It does not:

- prove the commit is trusted, signed, or safe, and it is not an approval step;
- lock the plan to a branch, remote, or author;
- include uncommitted or untracked source;
- guarantee the applied result still compiles or passes tests (`check` reports
  `behavior_checked: false`);
- stop someone from passing a different valid commit ID for the same plan.

It makes replay repeatable. It does not make it authorized or correct.

## Relationship to the unmerged 591184e fix

Commit `591184e` ("fix: record plan baseline on a fixed line") is on the
`vibe/pdev-197-guarded-planner` branch and is not merged into `main`. It changes
the create-plan and implement-plan prompts so they always write and read
`Baseline commit: <full SHA>` as a fixed line in Current State, instead of
leaving the baseline implicit or copying whatever `HEAD` happens to be.

The fix does not change Planner's Go code or replay behavior. The planner always
required a full `--base`; `591184e` only makes the correct value easier to find
and less likely to be replaced with `HEAD`.

## Who this matters to

For ordinary users who run Planner through the create-plan and implement-plan
prompts, the baseline is handled by the prompt and mostly invisible. The prompt
should record it once and reuse it.

It matters directly to maintainers of those prompts and to anyone writing or
running plan evaluations. An eval that creates a plan, commits an implementation
step, and then revises the plan will catch a workflow that used `HEAD`: the edit
fails because the diff no longer applies. When checking such a run, confirm that
every guarded command passed the original commit, not the new `HEAD`.
