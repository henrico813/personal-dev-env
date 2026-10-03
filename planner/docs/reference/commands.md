# Commands

Planner commands run from any directory; relative paths resolve against the
current directory.

## planner new

```text
planner new <output.md> [--issue --project NAME] [--diff] [--dry-run] [--json-errors]
```

Writes a new plan scaffold and fails without changing an existing destination.
The output path must end in `.md`.

- `--issue --project NAME` prepends the vault issue frontmatter. `--project` is
  required with `--issue` and rejected without it, and `date_created` is today's
  local date. The block matches the shape planner accepts, so a wrapped plan
  keeps parsing without further edits.
- `--diff` prints a review preview of the bytes that would be written. It is
  additive and still writes the file.
- `--dry-run` suppresses the write. Use `--diff --dry-run` to preview without
  writing; it exits 1 when the preview is non-empty.

## planner inspect

```text
planner inspect <plan.md>
planner inspect <plan.md> --target SELECTOR --repo DIR --base COMMIT [--code-out NEWFILE [--before]]
```

Without `--target`, inspect prints a JSON view of the parsed plan. With
`--target`, it is a guarded command.

`SELECTOR` is `implementation[N].file_changes[M]` with 1-based indices. Quote
it, because zsh expands unquoted brackets. Leading zeros are accepted and the
normalized selector is echoed in results.

`--code-out` writes the source after the selected change to a new scratch file,
or before it with `--before`. `--before` requires `--code-out`.

### Inspect result fields

- `selector`: normalized target.
- `filename`, `step_title`, `step_summary`, `explanation`: the selected change.
- `base`: the baseline passed in.
- `edit_expect`: token binding the plan bytes, normalized selector, and base.
- `validation`: `inspection_only`.
- `diff`: the selected diff, present without `--code-out`.
- `code_exists`, `code_state`, `mode`, `code_out`: present when source was
  exported.

## planner patch

```text
planner patch <plan.md> --target SELECTOR --expect TOKEN --repo DIR --base COMMIT
  (--after-file FILE | --diff-file FILE) [--dry-run] [--diff]
```

Patch replaces one fenced change. Exactly one of `--after-file` and
`--diff-file` is required.

- `--after-file FILE` generates a diff from ordinary source. Use `/dev/null` to
  propose deleting the file. Existing file mode is retained; a new file
  defaults to `100644`.
- `--diff-file FILE` imports a raw unified diff. `-` reads stdin.
- `--expect TOKEN` is the `edit_expect` from the targeted inspect.
- `--dry-run` validates without writing the plan.
- `--diff` prints a Git-generated review preview.

Patch replays the baseline plus every change through the edited one and never
later changes.

### Patch result fields

- `path`: the plan path.
- `plan_sha256`: hash of the updated plan.
- `written`: whether the plan was written.
- `structure_valid`, `patch_syntax_valid`: checks that ran.
- `prefix_replayed`: always `true` on success.
- `downstream_checked`: `false`; later changes are not replayed.
- `base`: the baseline passed in.
- `behavior_checked`: `false`.

## planner check

```text
planner check [<plan.md>] [--stdin] [--json-errors]
planner check <plan.md> --repo DIR --base COMMIT [--json-errors]
```

Without `--repo` and `--base`, check validates the plan structure, reports every
violation in one run, and prints `OK` on success. `--stdin` reads the plan from
stdin. `--format` is not accepted.

The guarded form opens a disposable repository at the baseline and replays the
whole plan in order. It reports `plan_sha256`, `structure_valid: true`,
`applicability_checked: true`, `changes_replayed`, `base`,
`source_state: "committed_snapshot_only"`, and `behavior_checked: false`.
`behavior_checked: false` means an applying patch is not proof that the result
compiles or passes tests.

## Validation modes

- Structure validation runs on parsed plan fields. It enforces required
  sections, non-empty fields, length limits, unique filenames per step, at
  least one goal, at least one implementation step, and at least one file
  change per step. It does not read source.
- Guarded replay validation applies every diff against the baseline in a
  disposable Git repository. It confirms applicability, not behavior.

## Frontmatter flag

`planner new --issue --project NAME` writes the vault issue frontmatter that the
parser accepts. The supported shape starts with `---` and contains a `tags` list
with `"#Ticket"`, `type: issue`, a `status` of `open`, `in-progress`, or `done`,
`template_version: 1`, a non-empty `project`, a `date_created` in `YYYY-MM-DD`
form, and a `topics` list. Any other wrapper is rejected. The frontmatter is
stripped before the plan body is parsed. As with a plain scaffold, `--diff`
alone still writes the plan; combine it with `--dry-run` to preview the
frontmatter without writing.

## Global flags

`--json-errors` emits failures as structured JSON to stderr.

## Exit codes

- `0`: success.
- `1`: read, decode, validation, source, or write failure.
- `2`: usage error.
