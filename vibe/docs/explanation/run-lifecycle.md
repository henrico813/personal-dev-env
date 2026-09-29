# Run lifecycle

## Identity prevents unrelated state from sharing a directory

A run identity combines the original key, its normalized slug, and a repository
ID made from the Git common directory. The slug gives stable branch and
worktree names. The repository hash prevents two checkouts with the same
basename from sharing run state. The stored original key prevents two keys that
normalize to one slug from silently sharing a branch or artifacts.

The key file is checked and written only after `run.lock` is acquired. This
ordering means two different keys racing for one slug cannot both claim it.
A missing key is unclaimed; the first successful locked run owns it.

## One record carries the run truth

`run.json` is written as phases advance and becomes terminal before derived
outputs are written. `summary.json` and `result.json` are views of that record,
while `runs_index.jsonl` is a best-effort lookup aid. If a derived write or
index append fails, `persistence_error` records the partial persistence issue
without replacing the execution status. Reading or writing `run.json`,
recording a later persistence failure, converting the record, or repairing an
index failure can return `Err`; the application then uses `fallback_result`.

## Stages share one finish path

After setup and `start_run`, the stage chain passes the values it has collected
to one finish path. That path writes the terminal record and derives the result,
so failures retain the pre-run commit and snapshots that were already available.
It also keeps `SnapshotFailed` distinct from wrapper failures: failure to read
the snapshot artifact is a snapshot failure, while the surrounding stage errors
remain wrapper failures.

## The 0.8.0 state-layout break

Version 0.8.0 added the Git-common-directory hash to state identity. The old
basename directory could combine repositories with the same name, so its runs
are intentionally orphaned rather than interpreted under the new identity.
There is no migration step; inspect old files directly when recovery requires
them.

See [state layout](../reference/state-layout.md) and [inspect a run](../how-to/inspect-a-run.md).
