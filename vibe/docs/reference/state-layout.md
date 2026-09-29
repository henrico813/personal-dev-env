# State layout

Vibe stores state below:

`~/.local/state/vibe/<basename>-<16 hex>/<slug>/`

`<basename>` is the target repository basename. The 16-hex suffix is the
FNV-1a 64-bit hash of the resolved Git common directory. The raw Git common
directory remains the path mounted for Git and Docker. `<slug>` is made from
the key by lowercasing, replacing each run of non-ASCII-alphanumeric
characters with `-`, trimming `-`, taking 48 characters, and using `vibe` when
empty.

Each slug directory contains:

| Path | Meaning |
| --- | --- |
| `key` | Original key that claimed this slug. |
| `run.lock` | Lock held while a run claims and uses the slug. |
| `runs_index.jsonl` | Best-effort index of run records. |
| `runs/<uuid>/run.json` | Authoritative run record. |
| `runs/<uuid>/summary.json` | Derived status summary. |
| `runs/<uuid>/result.json` | Derived saved command result. |
| `runs/<uuid>/prompt.txt` | Original prompt. |
| `runs/<uuid>/system-prompt.txt` | Rendered system prompt. |
| `runs/<uuid>/combined-prompt.txt` | Prompt sent to the executor. |
| `runs/<uuid>/system-prompt-versions.txt` | Prompt version manifest. |
| `runs/<uuid>/events.jsonl` | Raw Pi event stream. |
| `runs/<uuid>/agent.stderr.log` | Agent stderr captured by Vibe. |
| `runs/<uuid>/extension-events.jsonl` | Extension progress events. |
| `runs/<uuid>/snapshots.jsonl` | Snapshot records read after the agent. |
| `runs/<uuid>/vibe.log` | Vibe wrapper and index notes. |

`run.json` is authoritative. `summary.json` and `result.json` are derived from
it. `runs_index.jsonl` is only a lookup aid; status also scans the runs
directory and ignores index paths outside it.

The managed branch is `vibe/<slug>` and its worktree is
`<repo-root>/worktrees/<slug>`. State from the pre-0.8.0
`~/.local/state/vibe/<basename>/` layout is orphaned and is not migrated.

See [CLI reference](cli.md) for output fields and [run lifecycle](../explanation/run-lifecycle.md)
for why these boundaries exist.
