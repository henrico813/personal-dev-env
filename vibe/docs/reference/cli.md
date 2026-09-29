# CLI reference

## `vibe run`

`vibe run` executes one task in the managed worktree and prints one JSON
result. It must run from a Git checkout.

| Flag | Meaning |
| --- | --- |
| `--key <KEY>` | Stable task identity; required. |
| `--base <REVISION>` | Revision used only to seed a new managed branch and worktree. |
| `--prompt-file <PATH>` | UTF-8 prompt file; required. Relative paths use the current directory. |
| `--input <PATH>` | Repeatable absolute read-only Docker input mount. Relative paths use the current directory. |
| `--model <PROVIDER/MODEL>` | Model selector passed to Pi; required. |
| `--commit-message <TEXT>` | Commit message for dirty runs. |
| `--stderr-level <LEVEL>` | `error`, `warn`, `info`, `debug`, or `trace`; default `info`, also from `VIBE_STDERR_LEVEL`. |
| `--insecure-tls` | Disable TLS certificate verification inside Docker. |

## `vibe status`

| Flag | Meaning |
| --- | --- |
| `--key <KEY>` | Task identity to inspect; required. |
| `--long` / `-l` | Print the full `run.json` record instead of its derived summary. |

Both status forms require a target Git checkout.

## Status values and exit codes

| Status | Exit code |
| --- | ---: |
| `completed` | 0 |
| `noop` | 1 |
| `agent_failed` | 2 |
| `commit_failed` | 3 |
| `refused_dirty` | 4 |
| `snapshot_failed` | 5 |
| `wrapper_failed` | 6 |
| `setup_error` | 7 |

Setup errors include invalid inputs, an unreadable prompt, an invalid base
target, missing provider authentication, invalid skill roots, unavailable
Docker or runtime assets, worktree preparation failure, artifact creation
failure, repository-skill validation failure, a Git checkout failure, a
key collision, and an active run for the same slug. They return JSON with
`setup_error` and no `run.json`. The slug claim and `run.lock` may already
exist because the run command creates or checks them before entering
`app::execute`.

`vibe status` exits 2 when the current directory is not a target checkout, the
Git identity cannot be resolved, the stored key conflicts, the state cannot be
read, or no readable `run.json` exists.

## JSON output

`vibe run` prints these fields: `run_id`, `status`, `branch`, `worktree`,
`model`, `pre_run_commit`, `commit`, `snapshot_commits`, `artifacts_dir`,
`events_log_path`, `stderr_path`, `run_path`, `summary_path`, `changed_files`,
`persistence_error`, and `error_message`. Setup errors use null for unavailable
identity and path fields and an empty `changed_files` list.

Short `vibe status` prints the same run data as the derived summary, plus
`key`, `slug`, `created_at`, and `phase`. Its fields are `run_id`, `key`,
`slug`, `created_at`, `phase`, `status`, `branch`, `worktree`, `model`,
`pre_run_commit`, `commit`, `snapshot_commits`, `changed_files`,
`artifacts_dir`, `summary_path`, `result_path`, `events_log_path`,
`stderr_path`, `error_message`, and `persistence_error`.

`vibe status --long` prints the authoritative record with fields `run_id`,
`key`, `slug`, `created_at`, `phase`, `terminal_status`, `branch`, `worktree`,
`model`, `pre_run_commit`, `commit`, `snapshot_commits`, `changed_files`,
`artifacts_dir`, `run_path`, `summary_path`, `result_path`, `events_log_path`,
`stderr_path`, `error_message`, and `persistence_error`.

See [state layout](state-layout.md) for artifact locations and
[internals](internals.md) for the persistence boundaries.
