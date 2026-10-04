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
  --repo "$REPO" --base-commit "$BASE_COMMIT" \
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
  --repo "$REPO" --base-commit "$BASE_COMMIT" \
  --after-file /tmp/change-after
```

The token is tied to the exact plan file contents, the selector, and the base
commit. Any write to the plan makes it stale, so run inspect again before the
next patch. This includes a write to another change.

To capture the token without parsing JSON, use `--print edit_expect`:

```bash
TOKEN=$(planner inspect plan.md \
  --target 'implementation[1].file_changes[1]' \
  --repo "$REPO" --base-commit "$BASE_COMMIT" \
  --print edit_expect)
```

This prints only the raw token followed by a newline. Without `--print`, inspect
returns JSON. Use `/dev/null` to propose deleting
the file, or `--diff-file -` to import a raw unified diff from stdin. See the
[command reference](../reference/commands.md) for `--after-file` mode handling
and other patch flags.

When the file passed to `--after-file` matches the change's current source,
patch returns `changed: false` and does not write the plan. With `--diff`, the
preview is `No changes.` For a PLACEHOLDER change, submitting the exported file
unedited is an error: edit it first.

## Keep the base commit

Pass the full commit ID recorded when the plan was created as `--base-commit`. See
[Base commit replay](../explanation/base-commit-replay.md) for how the base commit is
recorded and why the original commit is required.

## Know what patch checked

`planner patch` replays the base commit plus every change through the edited one
and never later changes. On success it reports `prefix_replayed: true` and
`downstream_checked: false`. For whole-plan readiness, run:

```bash
planner check plan.md --repo "$REPO" --base-commit "$BASE_COMMIT"
```

## Keep at least one change per step

A step keeps at least one file change. To add or remove a change by hand, copy
a placeholder fence from the scaffold, fill it with `inspect --before` and
`patch`, then delete the old block.
