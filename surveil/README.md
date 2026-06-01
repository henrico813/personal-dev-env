# surveil

`surveil` works with structured task documents.

## Install

`pde install ai-tools` installs `~/.local/bin/surveil`.

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

- `surveil new task <output-dir>` writes a blank `task.md` stub at `<output-dir>/task.md` and fails if that file already exists.
- `surveil index --repo <repo>` builds a disposable Tantivy chunk index under `.surveil/index/` from readable UTF-8 files under the same repo skip policy used by `research`.
- `surveil gather --repo <repo> --task-file <task.md>` emits a versioned `GatherOutput` JSON context with `schema_version`.
- `surveil research --context <context.json> --trace-out <trace.json>` emits a versioned `ResearchOutput` JSON report with `schema_version` and writes a shallow `TraceOutput` JSON file.

## Output shape

Research results are grouped by query:
- `result`
- each answer has `query`, `findings`, and `negative_evidence`
- `findings` include `path`, `line`, `excerpt`, `source`, `matched_from`, and optional `symbol_kind`, `symbol_name`, `symbol_start_line`, and `symbol_end_line`

`research` remains lexical-first in its final output: it still derives every visible `Finding` from live file text and still fills symbol fields only when best-effort Tree-sitter enrichment succeeds.

A prebuilt `.surveil/index/` directory now participates in query-time ranking. At run start, `research` checks whether that index is usable against the repo-wide fingerprint recorded at build time, opens it once when it is, and reuses that reader for every query in the run without revalidating freshness. Each query still keeps explicit files first, scans top ranked files next, and expands to the rest of scope only when that first pass finds nothing. If the index is missing, stale, incompatible, or corrupt at startup, `research` falls back to the full scoped lexical scan.

`research` still emits only a small number of snippets per file, and the public result shape remains versioned via `schema_version`; each `result` entry includes `query`, `findings`, and `negative_evidence`, with optional symbol metadata on source-like findings.
