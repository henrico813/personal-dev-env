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

## Quick orientation

```bash
planner new plan.md
planner check plan.md
planner check plan.md --repo "$REPO" --base "$BASE"
```

`planner new` writes a scaffold and fails without changing an existing
destination. `planner check plan.md` validates the structure. The guarded form
adds `--repo` and `--base`; guarded `planner check` and `planner patch` replay
diffs from the recorded baseline.

## Issue frontmatter

PDE vault plans are issue documents, and PDE records the project, status, and
topics in YAML frontmatter. `planner new` writes a plain plan by default. Pass
`--issue --project <name>` to prepend that frontmatter. See the [command
reference](docs/reference/commands.md) for the supported fields and the preview
flags.

## Baseline commit

A plan's baseline is the full Git commit ID of the source repository when the
plan was created, and every fenced diff is measured against it. Pass it as
`--base` to the guarded commands. See [Baseline
replay](docs/explanation/baseline-replay.md) for how it is recorded and why the
original commit is required.

## Documentation

- [Create and check your first plan](docs/tutorials/create-and-check-a-plan.md)
- [Revise a guarded diff](docs/how-to/revise-a-guarded-diff.md)
- [Diagnose guarded failures](docs/how-to/diagnose-guarded-failures.md)
- [Commands, validation modes, frontmatter, and result fields](docs/reference/commands.md)
- [Baseline replay](docs/explanation/baseline-replay.md)

The full index is [docs/README.md](docs/README.md).
