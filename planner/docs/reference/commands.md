# Commands

Planner commands run from any directory; relative paths resolve against the
current directory.

## planner new

```text
planner new <output.md> [--issue --project NAME] [--diff] [--dry-run] [--json-errors]
```

Writes a new plan scaffold and fails without changing an existing destination.
The output path must end in `.md`.

- `--issue --project NAME` prepends the vault issue frontmatter. PDE vault
  plans are issue documents, so this frontmatter records the project, status,
  and topics for vault views. `--project` is required with `--issue` and
  rejected without it, and `date_created` is today's local date.
- `--diff` prints a review preview of the bytes that would be written. It is
  additive and still writes the file.
- `--dry-run` suppresses the write. Use `--diff --dry-run` to preview without
  writing; it exits 1 when the preview is non-empty, a diff-style status that
  says the preview has content to review, not that the command failed.

## planner inspect

```text
planner inspect <plan.md>
planner inspect <plan.md> --target SELECTOR --repo DIR --base-commit COMMIT [--code-out NEWFILE [--before]] [--print FIELD]
```

Without `--target`, inspect prints a JSON view of the parsed plan. With
`--target`, it is a guarded command.

`SELECTOR` is `implementation[N].file_changes[M]` with 1-based indices. Quote
it, because zsh expands unquoted brackets. Leading zeros are accepted and the
normalized selector is echoed in results.

`--code-out` writes the source after the selected change to a new scratch file,
or before it with `--before`. `--before` requires `--code-out`.
`--print edit_expect` prints only the raw token instead of JSON. The default
inspect output remains JSON; `edit_expect` is the only supported print field.

### Inspect result fields

- `selector`: normalized target.
- `filename`, `step_title`, `step_summary`, `explanation`: the selected change.
- `base_commit`: the base commit passed in.
- `edit_expect`: The token is tied to the exact plan file contents, the selector,
  and the base commit. Any write to the plan makes it stale, so run inspect again
  before the next patch.
- `validation`: `inspection_only`.
- `diff`: the selected diff, present without `--code-out`.
- `code_exists`, `code_state`, `mode`, `code_out`: present when source was
  written.

Without `--code-out`, targeted inspect returns the selected change's diff in
the JSON `diff` field. The selector chooses a single step even when earlier
steps change the same file:

```bash
planner inspect plan.md \
  --target 'implementation[2].file_changes[1]' \
  --repo "$REPO" --base-commit "$BASE_COMMIT" | jq -r .diff
```

`planner check` reports every structure and length violation in one run. Fix
all listed problems before rerunning it. For example, one response can include:

```json
{
  "code": "VALIDATE_INPUT",
  "message": "title must be no more than 66 characters (got 86)\noverview must be no more than 250 characters (got 335)\ndefinition_of_done.narrative must be no more than 250 characters (got 324)",
  "recovery_hint": "Fix the identified input or source assumption; do not retry unchanged."
}
```

## planner patch

```text
planner patch <plan.md> --target SELECTOR --expect TOKEN --repo DIR --base-commit COMMIT
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
- `--diff` prints the Git-generated review preview to stdout instead of the
  normal JSON result.
- If `--after-file` matches the selected change's current source, patch
  succeeds with `changed: false` and `written: false` and leaves the plan alone.
  With `--diff`, stdout says `No changes.` rather than returning empty output.
- For a PLACEHOLDER change, an unedited `--code-out` file fails with
  `after-file is identical to the --before file from inspect; edit it first`.

Patch starts from the base commit and applies every change through the edited
one; it never applies later changes.

### Patch result fields

- `path`: the plan path.
- `plan_sha256`: hash of the updated plan.
- `written`: whether the plan was written.
- `changed`: whether patch modified the plan. It is false when the submitted
  file matches the current source, and `written` is then false too.
- `structure_valid`, `patch_syntax_valid`: checks that ran.
- `prefix_applied`: always `true` on success; use this field.
- `prefix_replayed`: deprecated alias with the same value, kept for compatibility.
- `downstream_checked`: `false`; later changes are not applied.
- `base_commit`: the base commit passed in.
- `behavior_checked`: `false`.

## planner check

```text
planner check <plan.md> [--repo DIR] [--base-commit COMMIT] [--json-errors]
```

`planner check` validates the plan structure, then applies every change against
the base commit. If `--base-commit` is omitted, the first line of `### Current
State` must be `Base commit: <full commit ID>`. If `--repo` is omitted, Planner
uses the current working directory's Git repository. `--stdin` reads the plan
from stdin. `--format` is not accepted.

The check opens a disposable repository at the base commit and applies the
whole plan in order. It reports `plan_sha256`, `structure_valid: true`,
`applicability_checked: true`, `changes_applied`, `base_commit`,
`source_state: "committed_snapshot_only"`, and `behavior_checked: false`.
Use `changes_applied`. `changes_replayed` is a deprecated alias with the same
value, kept for compatibility.
`behavior_checked: false` means an applying patch is not proof that the result
compiles or passes tests.

When matches exist, JSON also includes `references_outside_plan`, a sorted list
of `{deleted_path, referencing_path, line, text}` records from literal path and
basename searches in the base commit. Changed files are excluded and warnings do
not change a successful exit. Results are bounded per deleted path;
`references_outside_plan_omitted` reports hidden matches.

If a diff fence still contains `PLACEHOLDER`, `planner check` fails with
`PATCH_INVALID` before Git runs. The message names the selector and file and
directs you to fill the change with `inspect --before` and `patch`. `planner patch`
runs the same check on earlier changes.

## planner export

```text
planner export <plan.md> --repo DIR --base-commit COMMIT --out NEWDIR
  [--through STEP] [--json-errors]
```

`planner export` applies the plan's diffs, in order, to a temporary copy of the
base commit, then writes every tracked file, including unchanged ones, to a new
directory. Unlike `inspect --code-out`, which writes one source file, it writes
a whole tree.

Use `--through STEP` to include step STEP and every earlier step. STEP is the
number N in an implementation heading `### N.`. Without it, every step is
included.

If a diff does not apply or writing fails, no output directory is left behind.
The output directory must not already exist. Export keeps executable modes
and symlinks, omits files the plan deletes, and writes submodule entries as
empty directories. The tree is the base commit plus the plan's changes, so
files the plan adds are included. Uncommitted and untracked files in your
checkout are not. Files are read straight from Git objects, so no code, tests,
hooks, or filters run.

Like check and patch, export applies diffs to the base commit and reports
failures the same way; see [Diagnose guarded failures](../how-to/diagnose-guarded-failures.md).

## Validation modes

- Structure validation runs on parsed plan fields. It enforces required
  sections, non-empty fields, length limits, unique filenames per step, at
  least one goal, at least one implementation step, and at least one file
  change per step. It does not read source.
- Guarded apply validation applies every diff against the base commit in a
  disposable Git repository. It confirms applicability, not behavior.

## Issue frontmatter

PDE vault plans are issue documents, and the vault tracks each one through YAML
frontmatter. `planner new` writes a plain plan by default; pass `--issue
--project NAME` to prepend that frontmatter. The supported shape starts with
`---` and contains a `tags` list with `"#Ticket"`, `type: issue`, a `status` of
`open`, `in-progress`, or `done`, `template_version: 1`, a non-empty `project`,
a `date_created` in `YYYY-MM-DD` form, and a `topics` list. Any other wrapper is
rejected. The frontmatter is stripped before the plan body is parsed. As with a
plain scaffold, `--diff` alone still writes the plan; combine it with
`--dry-run` to preview the frontmatter without writing.

## Global flags

`--json-errors` emits failures as structured JSON to stderr.

## Exit codes

- `0`: success.
- `1`: read, decode, validation, source, or write failure. `--diff --dry-run`
  also exits 1 for a non-empty preview as a diff-style status, not a failure.
- `2`: usage error.
