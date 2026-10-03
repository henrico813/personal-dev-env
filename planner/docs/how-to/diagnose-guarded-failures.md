# Diagnose guarded failures

Guarded commands emit failures as a JSON object on stderr when `--json-errors`
is set:

```json
{"code": "...", "message": "...", "recovery_hint": "..."}
```

Without the flag they print `planner: CODE: message`. The process exits 2 for a
usage error and 1 for other failures.

## Read the code first

The code names the failure category:

- `USAGE`: bad flag combination or selector.
- `READ_INPUT`, `DECODE_INPUT`, `VALIDATE_INPUT`: the plan, diff, or scratch
  input could not be read, parsed, or validated.
- `WRITE_OUTPUT`: the plan or exported source could not be written to disk.
- `SOURCE_CHECK`: the baseline could not be opened or a diff did not apply.
- `PATCH_INPUT`: the replacement source or diff was rejected before plumbing.
- `PLAN_STALE`: the `edit_expect` token did not match. Reread the plan and
  reconcile changes. Never refresh only the token and retry an old replacement.
- `PLAN_BUSY`: another writer holds the plan lock. Wait for the active writer,
  and remove a leftover lock only after confirming no writer is active.
- `VALIDATE_RESULT`, `PLAN_EDIT`, `PLAN_COLLATERAL_CHANGE`: the replacement
  changed the plan in an unsupported way.
- `RUNTIME`: an untyped failure that the CLI could not classify.
- `OUTPUT_REPORT_FAILED`: the result could not be reported. The plan may already
  have been written, so inspect it before retrying.

The patch engine also reports `BASE_REQUIRED`, `BASE_UNAVAILABLE`,
`UNSUPPORTED_PATH`, `PATCH_INVALID`, `PATCH_ENVELOPE`, `PATCH_PATH_MISMATCH`,
`PATCH_UNSUPPORTED`, `PATCH_NOT_APPLICABLE`, `PATCH_NO_CHANGE`, and
`PLAN_UNSUPPORTED` when a guarded operation reaches it.

## Common causes

- `--base` is `HEAD`, a branch name, or a tag. Use the full commit ID recorded
  when the plan was created.
- Diffs were written against the wrong commit, so a later change expects
  content that is not present.
- The plan was edited after inspect, so `edit_expect` no longer matches.
- A step's diff no longer applies because an earlier change was edited without
  replaying the prefix.
