# Baseline replay

A plan's baseline commit is the full Git commit ID of the source repository at
the moment the plan was created. Every fenced diff in the plan describes a
change measured against that commit.

## Why guarded commands need the original commit

Each diff describes how to change a file as it existed at the baseline.
`planner patch` and guarded `planner check` rebuild that starting point in a
disposable Git repository, then apply diffs on top of it. Guarded `planner
inspect` opens and replays the baseline only when `--code-out` is requested;
without it, inspect computes the `edit_expect` token and returns the selected
diff without opening the baseline.

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
implementation step is then committed, moving `HEAD` forward:

```text
A  plan created; diffs measured against A
B  step 1 committed
C  step 2 committed        <- HEAD is now C
```

The plan's diffs still describe changes from A to B and from B to C. Replay must
therefore start at A and apply each diff in order:

```text
base A -> apply step 1 -> apply step 2
```

If a guarded command starts at C instead, step 1's diff expects the file as it
was at A, but the file at C already contains step 1's changes. The apply fails,
or succeeds against content that only happens to match. Pass A, not C.

## What Planner does with the baseline

`--base` is used in three places:

- `planner inspect` without `--code-out` computes the `edit_expect` token and
  returns the selected diff; it does not open the baseline. With `--code-out`,
  it opens a disposable repository at the baseline, replays every change before
  the selected one, and exports the source just before or just after that
  change. The returned `edit_expect` token is tied to the plan bytes, the
  normalized selector, and the baseline.
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

## Where the baseline is recorded

The create-plan and implement-plan prompts record and read the full baseline
commit in `### Current State` of the plan. Planner does not parse that text; the
caller reads the recorded value and passes it as `--base` to the guarded
commands. Planner never defaults to `HEAD`. Choosing the commit recorded when
the plan was created is a prompt and workflow convention, not a value Planner
enforces; Planner uses only the full commit ID supplied by the caller as
`--base`.

## Who this matters to

When you run Planner through the create-plan and implement-plan prompts, the
prompt records the baseline once and reuses it, so the value is mostly handled
for you. Maintainers of those prompts and anyone testing the plan workflow need
to watch it directly. A test that creates a plan, commits an implementation
step, and then revises the plan fails if the workflow used `HEAD`, because the
diff no longer applies. When checking such a run, confirm that every guarded
command passed the original commit, not the new `HEAD`.
