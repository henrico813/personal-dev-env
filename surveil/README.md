# surveil

`surveil` works with strict JSON task documents.

## Install

`pde-installer install` installs `~/.local/bin/surveil`.
It also uses a scoped chezmoi modifier to merge an `allow` rule for the complete Surveil state directory
into `~/.config/opencode/opencode.json`, so managed research artifacts do
not trigger external-directory prompts. The rule follows an absolute
`XDG_STATE_HOME` and otherwise targets `~/.local/state/surveil/**`.

## Task format

```json
{
  "summary": "short summary text",
  "explicit_files": ["path/to/file.rs"],
  "search_areas": ["src", "docs"],
  "query": ["What changed?", "What still needs verification?"],
  "terms": ["optional", "keywords"]
}
```

Notes:
- Unknown, missing, null, or incorrectly typed fields are rejected.
- `summary` must not be blank.
- `search_areas` must contain at least one path.
- `query` must contain at least one nonblank question.
- `explicit_files` and `terms` may be empty arrays.
- Relative explicit files and search areas resolve against `--repo`; use `.` for the repository root.
- Absolute paths are used unchanged, and search areas may name files or directories.

## Commands

- `surveil new task <output-dir>` writes a task JSON stub with all five fields at `<output-dir>/task.json` and fails if that file already exists.
- `surveil new task --task <name>` collision-safely reserves a unique managed root, writes `<name>/task.json`, and prints the root path after the command completes.
- `surveil new task --root <root> --task <name>` collision-safely reserves a new task leaf in an existing absolute Surveil-managed root, writes `task.json`, and prints that root after completion. Duplicate task names fail without changing the existing task; failed writes remove the newly reserved leaf.
- `surveil index --repo <repo>` builds a disposable Tantivy chunk index under `.surveil/index/` from readable UTF-8 files under the same repo skip policy used by `research`.
- `surveil gather --repo <repo> --task-file <task.json>` emits a `surveil.v7` `GatherOutput` JSON context. Its required `task_name` is the UTF-8 name of the resolved `task.json` parent directory.
- `surveil research --context <context.json> --trace-out <trace.json>` requires a `surveil.v7` context, propagates `task_name` into a `surveil.v7` report, and writes a shallow `TraceOutput` JSON file.
- `surveil merge <task-report>...` accepts positional report paths, validates `surveil.v7` task reports, rejects invalid or duplicate `task_name` values, and emits one `surveil.evidence.v2` evidence pack.

Task names may contain ordinary spaces and Unicode, but must not be empty or whitespace-only and must not contain controls, path separators, or path components such as `.` and `..`. Managed creation, gather-derived identity, and merge report loading apply the same rules.

Managed roots live under `$XDG_STATE_HOME/surveil/runs` when `XDG_STATE_HOME` is absolute. Otherwise Surveil uses `$HOME/.local/state/surveil/runs`, which requires an absolute `HOME`. Environment paths retain their native operating-system representation. Managed roots remain until a caller removes them; Surveil does not prune them automatically.

The `.surveil-managed` marker prevents accidental append to unrelated directories. It is misuse prevention, not protection against a hostile filesystem. Root and task directories are reserved with `create_dir`, but a complete task is published only when the creating command successfully writes `task.json`.

Report v7 and evidence v2 intentionally replace v6 and v1. Research rejects non-v7 gather contexts, and merge rejects older reports rather than upgrading either format.

Default skipped path segments are `target`, `node_modules`, `dist`, `build`, `pack`, `.git`, `.surveil`, `.spendscope`, `worktrees`, `.venv`, `venv`, `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, `.tox`, `.nox`, `htmlcov`, `.next`, `.nuxt`, `.svelte-kit`, and `.turbo`. The same skip policy applies to `index`, `gather`, and `research`.

## Output shape

Research results are grouped by query:
- `result`
- each answer has `query`, `findings`, and `negative_evidence`
- `findings` include `path`, `line`, `excerpt`, `source`, `matched_from`, and optional `symbol_kind`, `symbol_name`, `symbol_start_line`, and `symbol_end_line`

`research` remains lexical-first in its final output: it still derives every visible `Finding` from live file text and still fills symbol fields only when best-effort Tree-sitter enrichment succeeds.

A prebuilt `.surveil/index/` directory now participates in query-time ranking. At run start, `research` checks whether that index is usable against the repo-wide fingerprint recorded at build time, opens it once when it is, and reuses that reader for every query in the run without revalidating freshness. Each query still keeps explicit files first, scans top ranked files next, and expands to the rest of scope only when that first pass finds nothing. If the index is missing, stale, incompatible, or corrupt at startup, `research` falls back to the full scoped lexical scan.

`research` still emits only a small number of snippets per file, and the public result shape remains versioned via `schema_version`; each `result` entry includes `query`, `findings`, and `negative_evidence`, with optional symbol metadata on source-like findings.

Merged reports, findings, occurrences, negative evidence, blockers, and open questions retain first-seen order. Findings with the exact same `path`, `line`, and `excerpt` share one entry; ordered `occurrences` describe each distinct task report and query appearance with a one-based rank. `EvidenceReport`, `FindingOccurrence`, `QueryEvidenceNote`, and `ReportEvidenceNote` preserve the report's `task_name`. Exact duplicate notes and occurrences are omitted, and input report paths are never copied into output.

`merge` validates JSON shape and schema version, but reports do not identify a repository revision. Callers are responsible for supplying reports produced from the same source snapshot.
