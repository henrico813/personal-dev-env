# Moshi phone approval reference

The phone ask is started as:

```text
moshi-hook ask --require-remote --source pde-gh-write --timeout 4h QUESTION
```

The request state remains under `~/.local/state/pde/pr-write/`. Each request
record stores the detached ask PID while it is running. If the recorded process
is gone, a rerun starts a new ask; a live PID prevents a second ask. The ask
clears the PID when it exits: after recording an answer, or when it times out
or gets no answer. An answer from Herdr or a terminal, a finished run, or
expiry sends SIGTERM to the ask's process group.

The question starts with a line such as `Approve merge #185?` (`Approve create
PR?` without a target), followed by one line each for `owner/repo` (or
`dir: NAME`), the Herdr workspace and session title, and the PR title: the
same lines the popup starts with. Lines without a value are left out.
