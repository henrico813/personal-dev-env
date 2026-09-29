# Inspect a run

1. From the target checkout, run:

   `vibe status --key <key>` and `vibe status --key <key> --long`

   The short form shows the derived summary. The long form shows the full
   authoritative record. Both print JSON; a missing or conflicting state exits
   2. See [CLI reference](../reference/cli.md) for fields and status codes.

2. Read `artifacts_dir` from the JSON, then inspect `run.json`,
   `summary.json`, `result.json`, `events.jsonl`, `agent.stderr.log`,
   `extension-events.jsonl`, `snapshots.jsonl`, and `vibe.log` in that run
   directory. The complete file list is in [state layout](../reference/state-layout.md).

3. Interpret `status` and the process exit code together. `status` describes
   execution; the exit code is the stable numeric mapping. A non-null
   `persistence_error` means Vibe could not fully persist run outputs or metadata,
   such as a derived file, index, changed-files collection, or emitted result
   file; it does not change the execution status.

4. If the run predates version 0.8.0, look below:

   `~/.local/state/vibe/<basename>/<slug>/runs/`

   That old basename directory is orphaned and is not read by current status
   lookup.
