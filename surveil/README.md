# surveil

`surveil` works with structured task documents.

## Task format

```md
# Task

## Summary
short summary text

## Explicit Files
- path/to/file.rs

## Search Areas
- src/
- docs/

## Query
- What changed?
- What still needs verification?

## Terms
- optional keywords
```

Notes:
- `# Task` is the title and is ignored by the parser.
- Only the `##` sections above are interpreted.
- `## Query` is required.
- `## Terms` is optional.

## Commands

- `surveil gather --repo <repo> --task-file <task.md>` emits a versioned `GatherOutput` JSON context with `schema_version`.
- `surveil research --context <context.json> --trace-out <trace.json>` emits a versioned `ResearchOutput` JSON report with `schema_version` and writes a shallow `TraceOutput` JSON file.
- `research` rejects context files whose `schema_version` does not match the current version instead of migrating them.

## Output shape

Research results are grouped by query:
- `result`
- each answer has `query`, `findings`, and `negative_evidence`
- `findings` include `path`, `line`, `excerpt`, `source`, `matched_from`, and optional `symbol_kind`, `symbol_name`, `symbol_start_line`, and `symbol_end_line`

`research` is lexical-first: it scans readable UTF-8 files for token matches, then uses tree-sitter symbol metadata when the file extension is `rs`, `go`, `py`, `ts`, or `tsx` and parsing succeeds. Unsupported extensions or unparseable UTF-8 files keep symbol fields `null`. Unreadable or non-UTF-8 files are skipped and listed in `TraceOutput.skipped_paths`.

`research` prefers declared `Explicit Files`, ranks candidate files before flattening findings, and emits only a small number of snippets per file. The JSON shape stays the same.
