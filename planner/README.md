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
