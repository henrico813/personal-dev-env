# Internals

This page describes the implementation for contributors.

## Module map

- `main.rs`: command dispatch, run lock, emitted JSON, and exit status.
- `app.rs`: setup, staged execution, and terminal result finishing.
- `cli.rs`: Clap arguments and path normalization.
- `target.rs`: key, slug, repository identity, and derived paths.
- `worktree.rs`: claims, branches, managed worktrees, and Git inspection.
- `ledger.rs`: run records, phases, derived outputs, and persistence.
- `observe.rs`: run artifact paths and prompt artifacts.
- `result.rs`: output status values and exit codes.
- `sandbox.rs`: runtime assets and agent execution.
- `snapshot.rs`: snapshot JSONL parsing.
- `adapters/`: Git, Docker, and runtime integrations.

## Identity and ledger

`RunTarget` preserves the raw Git common directory for mounts and hashes its
resolved path for the state repository ID. It supplies the key, slug, branch,
worktree, state directory, and claim path.

The ledger API starts a record with `start_run`, updates phases with
`persist_phase`, stores the pre-run commit with `persist_pre_run_commit`, and
finishes through `persist_terminal_run`. `TerminalOutcome` carries stage values;
`TerminalOutcome::failure` clears result-commit and changed-file fields.
`RunSummary::from` derives the status view. `RunResult::try_from` derives an
emitted result only from a record with a terminal status.

`persist_terminal_run` reads and writes `run.json` before attempting
`summary.json` and `runs_index.jsonl`. A summary failure is recorded in
`run.json`; if that update succeeds, a result is returned. An index failure is
reported as a late persistence error and its repair is attempted. Reading or
writing `run.json`, recording a later failure, converting the record, or
repairing an index failure can return `Err`; `app` then builds the result with
`fallback_result`.

## Phases and stage chain

The phases are ordered as `preparing_artifacts`, `copying_prompt`,
`checking_dirty`, `reading_pre_run_commit`, `preparing_sandbox`,
`running_agent`, `reading_snapshots`, `committing_result`, and `finished`.
The stage note appended to `vibe.log` is, in order: `artifacts prepared`,
`copy prompt`, `check dirty`, `read pre-run commit`, `prepare sandbox`,
`run agent`, `read snapshots`, and `commit result`.

`prepare_active_run` performs setup and creates the artifact directory.
`prepare_stage` writes prompt artifacts, checks dirtiness, records the
pre-run commit, and validates repository skills. `agent_stage` runs the agent.
`snapshot_stage` reads snapshots and checks post-agent dirtiness. `finish_stage`
commits dirty results and collects changed files. `run_stages` preserves the
sequence and returns one terminal outcome. `execute` maps setup errors to a
setup result, returns a start failure without terminal persistence, and sends
every later outcome through `finish_result`.

| Failure point | Status | Pre-run commit | Snapshots |
| --- | --- | --- | --- |
| Setup before artifact creation | `setup_error` | not present | not present |
| `start_run` | `wrapper_failed` | not present | not present |
| Stage before pre-run commit is read | `wrapper_failed` or `refused_dirty` | not present | not read |
| Stage after pre-run commit, before snapshot read | `wrapper_failed` | kept | not read |
| Snapshot file read | `snapshot_failed` | kept | not kept |
| Stage after snapshots are read | `wrapper_failed` | kept | kept |
| Result commit attempt | `commit_failed` | kept | kept |

After snapshot processing succeeds, `finish_stage` chooses `noop` for a clean
zero exit, `completed` for a dirty zero-exit run whose commit succeeds, and
`agent_failed` for a non-zero agent exit when no commit failure replaces it.
A failed result commit produces `commit_failed`; all post-read outcomes retain
snapshot commits.

See [state layout](state-layout.md) for files and [CLI reference](cli.md) for
serialized output.
