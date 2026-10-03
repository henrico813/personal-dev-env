# Create and check your first plan

This tutorial creates a plan, checks its structure, and checks that its diffs
replay against a source commit. It uses a repository at `$REPO` and a full
commit ID `$BASE` for the baseline.

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

Write the diffs against the source as it exists at the baseline. Do not change a
code diff by hand once it is part of a guarded revision; use [Revise a guarded
diff](../how-to/revise-a-guarded-diff.md) instead.

## 3. Check the structure

```bash
planner check plan.md
```

Without `--repo` and `--base`, `planner check` validates the plan structure and
prints `OK` on success. It does not read the source.

## 4. Check the diffs against the baseline

Record the full commit ID of the source repository at the time the plan was
created, then pass it as `--base`:

```bash
planner check plan.md --repo "$REPO" --base "$BASE"
```

The guarded form opens a disposable repository at the baseline, replays every
change in order, and confirms each diff applies. It reports
`source_state: "committed_snapshot_only"` and `behavior_checked: false`, because
an applying patch is not proof that the result compiles or passes tests.

Read the baseline recorded in Current State and pass it as `--base`. See
[Baseline replay](../explanation/baseline-replay.md) for why the original commit
is required.
