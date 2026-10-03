# Revise a guarded diff

The guarded commands check a plan's diffs against a source commit before
anything is written or reported ready. Revise a code diff through `planner
inspect` and `planner patch` so the change is replayed and the plan structure
stays valid.

## Inspect the change and export the source

Select the file change with a quoted selector and export the source after it:

```bash
planner inspect plan.md \
  --target 'implementation[1].file_changes[1]' \
  --repo "$REPO" --base "$BASE" \
  --code-out /tmp/change-after
```

Quote the selector, because zsh expands unquoted brackets. Use `--before` to
export the source before the selected change instead of after it. Both forms
return an `edit_expect` token. See the [command
reference](../reference/commands.md) for selector and `--code-out` details.

## Edit and patch

Edit the exported file, then write the change back:

```bash
planner patch plan.md \
  --target 'implementation[1].file_changes[1]' \
  --expect "$TOKEN" \
  --repo "$REPO" --base "$BASE" \
  --after-file /tmp/change-after
```

`edit_expect` binds the plan bytes, the normalized selector, and the baseline,
so any edit to the plan invalidates it. Use `/dev/null` to propose deleting the
file, or `--diff-file -` to import a raw unified diff from stdin. See the
[command reference](../reference/commands.md) for `--after-file` mode handling
and the other patch flags.

## Keep the baseline

Pass the full commit ID recorded when the plan was created as `--base`. See
[Baseline replay](../explanation/baseline-replay.md) for how the baseline is
recorded and why the original commit is required.

## Know what patch checked

`planner patch` replays the baseline plus every change through the edited one
and never later changes. On success it reports `prefix_replayed: true` and
`downstream_checked: false`. For whole-plan readiness, run:

```bash
planner check plan.md --repo "$REPO" --base "$BASE"
```

## Keep at least one change per step

A step keeps at least one file change. To add or remove a change by hand, copy
a placeholder fence from the scaffold, fill it with `inspect --before` and
`patch`, then delete the old block.
