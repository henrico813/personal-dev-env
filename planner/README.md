# Planner

Planner is the planning CLI in this repository. It creates and revises plan
documents whose code changes are written as unified diffs. Some commands read
or write plan Markdown only; others check a proposed diff against a recorded
source commit before anything is written or reported ready.

## Document-only and guarded commands

Document-only commands never touch a source repository:

    planner new <output.md> [--issue --project NAME] [--diff] [--dry-run] [--json-errors]
    planner check [<plan.md>] [--stdin] [--json-errors]
    planner inspect <plan.md>

`planner check` without `--repo`/`--base` reports every structure violation in
one run. `planner inspect` without `--target` prints the parsed plan as JSON.

Guarded commands take `--repo` and `--base` together:

    planner inspect <plan.md> --target 'SELECTOR' --repo DIR --base COMMIT
        [--code-out NEWFILE [--before]] [--json-errors]
    planner patch <plan.md> --target 'SELECTOR' --expect TOKEN --repo DIR --base COMMIT
        (--after-file FILE | --diff-file FILE) [--dry-run] [--diff] [--json-errors]
    planner check <plan.md> --repo DIR --base COMMIT [--json-errors]

## The baseline commit

A plan's baseline is the full commit ID of the source repository at the moment
the plan was created. Every fenced diff describes a change measured against that
commit. The create-plan prompts record the full baseline in the plan's
`### Current State`, and the implement-plan prompts read it and pass it as
`--base`. Planner does not parse that text: the baseline is a convention between
the prompts and the caller for the source commit recorded at creation, not a
value Planner enforces. `--base` must be a full commit ID; `HEAD`, branch names,
and tags are rejected so the same bytes can still be found after the branch has
moved.

## Why replay starts at the recorded baseline

Plan creation reads the source at commit A and records A. Each implementation
step is then committed, moving `HEAD` forward:

    A  plan created; diffs measured against A
    B  implementation step 1 committed
    C  implementation step 2 committed    <- HEAD is now C

The plan's diffs still describe A to B and B to C, so replay must start at A and
apply each diff in order:

    base A -> apply step 1 -> apply step 2

Starting at C fails or matches the wrong content: step 1's diff expects the file
as it was at A, but the file at C already contains step 1's changes. A guarded
command given the current `HEAD` can then report readiness for a plan that does
not match the tree it will be applied to.

## What each guarded command does with the baseline

- `planner inspect` computes `edit_expect` from the plan bytes, the normalized
  selector, and the baseline without opening the baseline. With `--code-out` it
  opens a disposable repository at the baseline, replays every change before the
  selected one, and exports the source just before or just after that change.
  The JSON result carries `selector`, `filename`, `step_title`, `step_summary`,
  `explanation`, `base`, `edit_expect`, and `validation: "inspection_only"`,
  plus `diff`; with `--code-out` it adds `code_exists`, `code_state`, `mode`, and
  `code_out`.
- `planner patch` re-checks `edit_expect`, replays the baseline plus every change
  through the edited one, and only then writes the plan. It replays no later
  changes and reports `prefix_replayed: true`, `downstream_checked: false`, and
  `behavior_checked: false`. Add `--dry-run` to validate without writing or
  `--diff` for a Git-generated review preview.
- `planner check --repo --base` replays every diff from the baseline and reports
  `applicability_checked: true`, `changes_replayed`, `source_state` as
  `committed_snapshot_only`, and `behavior_checked: false`.

The disposable repository shares read-only Git objects with your repository and
is removed afterwards. Planner never stages, stashes, resets, or commits your
worktree, and uncommitted or untracked files are not part of the baseline.

## What the baseline does not protect against

The baseline identifies a starting point for replay. It does not:

- prove the commit is trusted, signed, or safe, and it is not an approval step;
- lock the plan to a branch, remote, or author;
- include uncommitted or untracked source;
- guarantee the applied result compiles or passes tests;
- stop someone from passing a different valid commit ID for the same plan.

It makes replay repeatable. It does not make it authorized or correct.

## Issue frontmatter

`planner new` writes a plain scaffold by default. When the plan belongs in the
PDE vault, pass `--issue --project <name>` to prepend the vault issue
frontmatter. `--project` is required with `--issue` and rejected without it,
`date_created` is today's local date, and `--diff`/`--dry-run` preview the same
bytes that would be written. The block matches what `planner check` accepts, so
a wrapped plan keeps parsing without further edits.
