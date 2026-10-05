# How planner applies diffs to the base commit

A plan's base commit is the full Git commit ID of the source repository at the
moment the plan was created. Every fenced diff in the plan describes a change
measured against that commit.

Planner applies each diff in the plan, in order, to a temporary copy of the base
commit.

## Why guarded commands need the original commit

Each diff describes how to change a file as it existed at the base commit.
`planner patch` and guarded `planner check` rebuild that starting point in a
disposable Git repository, then apply diffs on top of it. Guarded `planner
inspect` opens a copy of the base commit only when `--code-out` is requested;
without it, inspect computes the `edit_expect` token and returns the selected
diff without opening the base commit.

`--base-commit` must be a full commit ID. `HEAD` is rejected, and so are branch
names and tags. The reason is not authentication; it is that the same bytes have
to be found later, even after the branch has moved.

If you pass the current `HEAD` instead of the recorded base commit:

- diffs for earlier steps no longer apply, because those changes are already in
  `HEAD`;
- a diff can apply against the wrong content and verify the wrong source;
- `check` can report readiness for a plan that does not match the tree it will
  actually be applied to.

## A/B/C example: why applying starts at A

Plan creation reads the source at commit A and records A as the base commit. Each
implementation step is then committed, moving `HEAD` forward:

```text
A  plan created; diffs measured against A
B  step 1 committed
C  step 2 committed        <- HEAD is now C
```

The plan's diffs still describe changes from A to B and from B to C. Applying must
therefore start at A and apply each diff in order:

```text
base A -> apply step 1 -> apply step 2
```

If a guarded command starts at C instead, step 1's diff expects the file as it
was at A, but the file at C already contains step 1's changes. The apply fails,
or succeeds against content that only happens to match. Pass A, not C.

## What Planner does with the base commit

`--base-commit` is used in three places:

- `planner inspect` without `--code-out` computes the `edit_expect` token and
  returns the selected diff; it does not open the base commit. With `--code-out`,
  it opens a disposable repository at the base commit, applies every change before
  the selected one, and writes the source just before or just after that
  change. The returned `edit_expect` token is tied to the plan bytes, the
  normalized selector, and the base commit.
- `planner patch` re-checks that token, starts from the base commit and applies
  every change through the edited one, and only then writes the plan. Later
  changes are not applied here.
- `planner check` applies the whole plan from the base commit to confirm every diff
  applies in order.

The disposable repository shares read-only Git objects with your repository and
is removed afterwards. Planner never stages, stashes, resets, or commits your
worktree, and uncommitted or untracked files are not part of the base commit.

## What the base commit does not protect against

The SHA identifies a starting point for applying diffs. It does not:

- prove the commit is trusted, signed, or safe, and it is not an approval step;
- lock the plan to a branch, remote, or author;
- include uncommitted or untracked source;
- guarantee the applied result still compiles or passes tests (`check` reports
  `behavior_checked: false`);
- stop someone from passing a different valid commit ID for the same plan.

It makes diff application repeatable. It does not make it authorized or correct.

## Where the base commit is recorded

The create-plan and implement-plan prompts record the full base commit in the
first line of `### Current State`. `planner check` reads that line when
`--base-commit` is not supplied. `planner inspect` and `planner patch` require
the caller to pass the full commit ID with `--base-commit`. Planner never uses
`HEAD` as a default.

## Who this matters to

When you run Planner through the create-plan and implement-plan prompts, the
prompt records the base commit once and reuses it. A test that creates a plan,
commits an implementation step, and then revises the plan fails if the workflow
uses `HEAD`, because the diff no longer applies. Check that guarded commands use
the recorded base commit, not the new `HEAD`.
