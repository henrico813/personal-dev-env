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

## Output shape

Research results are grouped by query:
- `result`
- each answer has `query`, `findings`, and `negative_evidence`
- `findings` include `path`, `line`, `excerpt`, `source`, `matched_from`, and optional `symbol_kind`, `symbol_name`, `symbol_start_line`, and `symbol_end_line`

`research` is lexical-first: it scans readable UTF-8 files for token matches, then best-effort enriches matches with Tree-sitter symbol metadata when parsing succeeds. Files that do not parse cleanly or do not yield a containing symbol keep symbol fields `null`. Unreadable or non-UTF-8 files are skipped and listed in `TraceOutput.skipped_paths`.

`research` prefers declared `Explicit Files`, ranks candidate files before flattening findings, and emits only a small number of snippets per file. It scans readable UTF-8 files lexically first, then best-effort enriches matches with Tree-sitter symbol metadata when parsing succeeds. The public result contract is versioned via `schema_version`; each `result` entry includes `query`, `findings`, and `negative_evidence`, with optional symbol metadata on findings.
