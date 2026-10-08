# Moshi phone approval reference

The phone ask is started as:

```text
moshi-hook ask --require-remote --source pde-gh-write --timeout 4h QUESTION
```

The request state remains under `~/.local/state/pde/pr-requests/`. Each request
has a `.moshi-REQUEST_ID.pid` marker while its detached ask is running. The
marker is removed when the ask exits or another approval path handles the
request.

The question uses `--repo OWNER/NAME` when supplied. Otherwise it uses the
read-only `gh repo view --json nameWithOwner -q .nameWithOwner` lookup from the
current directory. It leaves out `titled ...` when no title was supplied.
