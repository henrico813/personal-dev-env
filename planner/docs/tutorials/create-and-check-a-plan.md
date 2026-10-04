# Create and check your first plan

This tutorial creates a plan and checks that its diffs apply to a source
commit. It uses a repository at `$REPO` and a full commit ID `$BASE_COMMIT` for
the base commit.

## 1. Create a plan

Run `planner new` with a new Markdown path:

```bash
planner new plan.md
```

`planner new` fails without changing an existing destination. When the plan
belongs in the PDE vault, pass `--issue --project <name>` to prepend the vault
issue frontmatter. See the [command reference](../reference/commands.md) for
the supported shape.

The scaffold contains a title, overview, definition of done, at least one
implementation step, and verification. See the [command
reference](../reference/commands.md) for the fields and limits.

## 2. Write the plan

Edit the prose and structure directly in the Markdown file. Each implementation
step holds at least one file change, and each file change is a filename, a
required `>` explanation line, and a fenced unified diff.

Write the diffs against the source as it exists at the base commit. Do not
change a code diff by hand once it is part of a guarded revision; use [Revise a
guarded diff](../how-to/revise-a-guarded-diff.md) instead.

## 3. Record the base commit

Record the full source commit in the first line of `### Current State`:

    Base commit: <full 40-character or 64-character commit ID>

`planner check` reads this line when `--base-commit` is not supplied. It uses
the current directory's repository when `--repo` is not supplied.

## 4. Check the plan

Run the whole-plan check:

```bash
planner check plan.md --repo "$REPO" --base-commit "$BASE_COMMIT"
```

The check opens a disposable repository at the base commit, replays every
change in order, and confirms each diff applies. It reports
`source_state: "committed_snapshot_only"` and `behavior_checked: false` because
an applying patch is not proof that the result compiles or passes tests.

To build or test the proposed tree, write it to a new directory, then run your
build or tests there. Export does not run them:

```bash
planner export plan.md --repo "$REPO" --base-commit "$BASE_COMMIT" --out /tmp/proposed-tree
```

See [Base commit replay](../explanation/base-commit-replay.md) for why the
original commit is required.
