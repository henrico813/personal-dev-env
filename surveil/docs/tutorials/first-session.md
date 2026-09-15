# Run Your First Session

This tutorial creates one managed task, researches a repository, and inspects the completed session receipt.

## Before You Start

Use an installed `surveil` binary. Create a disposable repository with known files so the walkthrough is reproducible:

```bash
REPO=$(mktemp -d)
mkdir -p "$REPO/src"
printf '# Demo\n' > "$REPO/README.md"
printf 'pub fn run_session() {}\n' > "$REPO/src/lib.rs"
ROOT=$(surveil new task --task architecture)
```

`ROOT` is the absolute managed root printed by Surveil.

## Define the Task

Replace `$ROOT/architecture/task.json` with:

```json
{
  "summary": "Describe the repository architecture",
  "explicit_files": ["README.md"],
  "search_areas": ["src"],
  "query": ["How are the main components connected?"],
  "terms": ["module", "command", "state"]
}
```

Paths in the task resolve against `REPO`.

## Run the Session

```bash
RECEIPT=$(surveil session run --repo "$REPO" --root "$ROOT")
test -f "$RECEIPT"
```

Surveil uses a usable existing `.surveil/index/` when it is fresh, compatible, and readable. This first run has no index and completes with the lexical scan.

## Inspect the Result

```bash
python -m json.tool "$RECEIPT"
python -m json.tool "$ROOT/.surveil-session/evidence.json"
```

The receipt lists each context, trace, report, and the evidence file with its task identity, relative path, byte length, and SHA-256 digest. `receipt.json` does not list itself. A failure before publication creates no `.surveil-session` path; the completed receipt on disk is authoritative if writing its path to stdout later fails.

Remove the disposable repository when finished:

```bash
rm -rf "$REPO"
```

Next, use [Run a Research Session](../how-to/run-a-session.md) for repeatable operational steps and [Session Run Command](../reference/session-run.md) for exact behavior.
