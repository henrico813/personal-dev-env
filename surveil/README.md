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
- `surveil index --repo <repo>` builds `.surveil/index.sqlite` from readable UTF-8 files under the same repo skip policy used by `research`.
- `surveil gather --repo <repo> --task-file <task.md>` emits a versioned `GatherOutput` JSON context with `schema_version`.
- `surveil research --context <context.json> --trace-out <trace.json>` emits a versioned `ResearchOutput` JSON report with `schema_version` and writes a shallow `TraceOutput` JSON file.

## Output shape

Research results are grouped by query:
- `result`
- each answer has `query`, `findings`, and `negative_evidence`
- `findings` include `path`, `line`, `excerpt`, `source`, `matched_from`, and optional `symbol_kind`, `symbol_name`, `symbol_start_line`, and `symbol_end_line`

`research` remains lexical-first: it still searches only declared `Explicit Files` plus `Search Areas`, still uses the current substring matcher to build findings, and still fills symbol fields only when best-effort Tree-sitter enrichment succeeds. When `.surveil/index.sqlite` is present and fresh for a candidate file, `research` may load cached text from it instead of rereading the file. If the cache is missing, stale, or invalid, `research` falls back to direct file reads.

`research` prefers declared `Explicit Files`, ranks candidate files before flattening findings, and emits only a small number of snippets per file. The public result contract is versioned via `schema_version`; each `result` entry includes `query`, `findings`, and `negative_evidence`, with optional symbol metadata on source-like findings.
