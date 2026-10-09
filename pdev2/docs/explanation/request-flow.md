# How a Request Runs gh Once

A pull request cannot be deleted, so a duplicate comment or PR is permanent.
Agents also rerun a command to learn its result. The program therefore keeps
one record per request and makes sure `gh` runs at most once for it.

## One Record per Request

`pde-gh-write` hashes the quoted command, the title, and the body into a
request ID. The record lives in `pde/pr-write/ID.json` under the state
directory. Running the same command with the same text finds the same record,
so a rerun reports the earlier result instead of asking again. Changing any
of the three makes a new request that needs its own answer.

## States

A new record starts as `pending`. `pde-pr-approve` changes it to `approved`
or `declined`. Before starting `gh`, `pde-gh-write` saves `running`; after
`gh` exits it saves `ran` with the exit code. Every save writes a temporary
file and renames it over the record, so readers never see half a record.

## The Lock

Each request has a lock file next to its record, held with `flock`. Creating
the record, recording an answer, and running `gh` all happen while holding
it. Two identical first requests therefore cannot both create a record, and
two runs of an approved request cannot both start `gh`: the second waits for
the lock and then finds `ran`.

The lock stays held while `gh` runs. Finding `running` while holding the lock
therefore means the earlier run died before saving its result. That request
is reported as interrupted and never run again, because `gh` may already have
posted. The same happens if `gh` exits but the `ran` record cannot be saved.

## Waiting and Expiry

The writer polls the record for 90 seconds because agent commands are killed
after about two minutes. If no answer arrives, it exits 3 and the agent can
rerun the command after approval.

A record expires four hours after its last change. An expired record of any
state is replaced by a new pending one, so the same command needs a fresh
answer and may run again.

## Body Files Are Read Once

`--body-file PATH` and `-F -` are read once when the request is made. The text
is stored in the record and sent to `gh` on standard input, with the argument
changed to `--body-file -`. Editing the file after approval therefore cannot
change what gets posted. Because the text is part of the request ID, a
rewritten file is a new request.

## Showing pr edit Changes

For `pr edit`, the approver sees the current title and body next to the
requested ones as a diff. The current values come from `gh pr view`, which
runs while the lock is held, so it is stopped after 3 seconds. A failed
lookup is shown in the request instead of blocking it. When an unknown flag
comes before the PR number, the program cannot tell which argument is the PR,
so it says that instead of guessing.

Exact values are listed in the [reference](../reference/requests.md).
