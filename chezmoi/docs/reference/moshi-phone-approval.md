# Moshi phone approval reference

The phone ask is started as:

```text
moshi-hook ask --require-remote --source pde-gh-write --timeout 4h QUESTION
```

The request state remains under `~/.local/state/pde/pr-write/`. Each request
record stores the detached ask PID while it is running. If the recorded process
is gone, a rerun starts a new ask; a live PID prevents a second ask. The ask
clears the PID when it exits: after recording an answer, or when it times out
or gets no answer.

The question is `Approve gh pr OPERATION in REPO (TARGET)[, titled TITLE]?`.
REPO is the `--repo` value when given and otherwise `current repository`. The
`, titled TITLE` part is omitted when no title was supplied.
