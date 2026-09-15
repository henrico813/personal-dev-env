# Session Run Command

## Synopsis

```text
surveil session run --repo <REPO> --root <MANAGED_ROOT>
```

`--repo <REPO>` is an existing repository directory. Surveil resolves it to an absolute path and records that path in the receipt.

`--root <MANAGED_ROOT>` is a UTF-8 absolute existing root created by `surveil new task --task`. It must retain its `.surveil-managed` marker and must not be a symlink.

## Task Discovery

Surveil examines immediate child directories containing a direct regular `task.json`. Any immediate symlink entry is rejected, as is a symlinked task file. Names are validated and sorted lexically before execution. Task JSON content is validated later when that task's gather stage begins. `.surveil-session` is reserved for output; names such as `session`, `evidence.json`, and `receipt.json` remain valid task names.

## Output

On success, stdout contains only the absolute receipt path:

```text
<MANAGED_ROOT>/.surveil-session/receipt.json
```

The published tree is:

```text
.surveil-session/
├── tasks/<TASK>/context.json
├── tasks/<TASK>/trace.json
├── tasks/<TASK>/report.json
├── evidence.json
└── receipt.json
```

The `surveil.session.v1` receipt fields are:

| Field | Meaning |
| --- | --- |
| `schema_version` | Always `surveil.session.v1`. |
| `status` | Always `complete`. |
| `repo_root` | Resolved absolute UTF-8 repository path. |
| `task_names` | Task names in execution order. |
| `artifacts` | Ordered integrity records for task artifacts and evidence. |

Each artifact record contains `kind`, `task_name`, `path`, `byte_len`, and `sha256`. Context, trace, and report records contain their task name; the evidence record contains `null`. Paths are relative to `.surveil-session/` and contain only normal components. `byte_len` and the lowercase 64-digit SHA-256 cover the exact JSON bytes, including the trailing newline. Records appear as context, trace, and report for each sorted task, followed by evidence. `receipt.json` is not included in `artifacts` because it cannot contain its own digest.

## Execution and Exit Status

Tasks run sequentially. Each task performs the existing gather and research operations, and the reports are merged with the existing merge validation. Each task independently evaluates index usability at its own research startup; a usable index participates in ranking and missing, stale, incompatible, or corrupt indexes use lexical fallback. Repository files and task documents are live inputs and must remain stable until exit.

Status `0` means publication and receipt-path output both succeeded. Runtime and I/O errors return status `1`; Clap usage errors return status `2`. Runtime failures before publication leave stdout empty, write an error to stderr, and create no new `.surveil-session` path. Cleanup is best-effort, so a `.surveil-session-*.tmp` sibling can remain. If stdout fails after the atomic no-replace move, status is `1` but the published receipt remains authoritative. Any existing `.surveil-session` file, directory, or symlink is an error and is never overwritten.

## V1 Limits

The command does not create tasks, build or refresh indexes, retry failed stages, produce degraded evidence, snapshot repository files, sign receipts, or run tasks concurrently.
