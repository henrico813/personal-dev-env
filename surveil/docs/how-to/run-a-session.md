# Run a Research Session

Use this procedure when a Surveil-managed root already contains populated task documents.

## Set the Paths

Use UTF-8 absolute paths for the repository and prepared managed root:

```bash
REPO=/absolute/path/to/repository
ROOT=/absolute/path/to/managed-root
test -f "$ROOT/.surveil-managed"
```

Populate every `<task>/task.json` before running the session. Empty generated stubs are invalid inputs. See [Run Your First Session](../tutorials/first-session.md) to create a root from scratch.

## Optionally Build the Index

For repeated research on a stable repository, build the disposable index first:

```bash
surveil index --repo "$REPO"
```

`session run` never creates or refreshes the index. Each task independently checks index usability when its research begins and otherwise scans its scoped files lexically.

## Execute and Check Completion

```bash
RECEIPT=$(surveil session run --repo "$REPO" --root "$ROOT")
test "$RECEIPT" = "$ROOT/.surveil-session/receipt.json"
test -f "$RECEIPT"
```

Keep both the repository and managed task documents stable until the command exits. Session V1 reads them live and does not create a snapshot.

## Handle Failure

After a nonzero exit, inspect `$ROOT/.surveil-session/receipt.json`. If it is absent, publication did not complete; correct the reported task, repository, or filesystem error and run again. Best-effort cleanup can leave a `.surveil-session-*.tmp` sibling that may require manual removal. If the receipt exists, publication completed and a later operation such as writing the receipt path to stdout failed.

Surveil refuses to overwrite any existing `.surveil-session` path, including a malformed file, directory, or symlink. Preserve valid output as evidence, or deliberately archive or remove the existing path before starting a new run from the same managed root.
