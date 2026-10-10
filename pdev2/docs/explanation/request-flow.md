# How a Request Runs gh Once

Coding agents post to your pull requests: they open them, comment, change
titles, and merge. A duplicate or unwanted post is visible to everyone and
has to be cleaned up by hand. Agents also rerun a command to find out what
happened to it. This program makes each post wait for your approval and
happen only once, however often the agent reruns the command.

`gh` is GitHub's command-line tool. A wrapper named the guard stops agents
from using it to write to a pull request, so they run `pde-gh-write` with
the same arguments instead.

## One Request, Start to Finish

1. An agent runs `pde-gh-write pr comment 12 --body "Looks good"`.
2. The program saves a record of the request and prints its ID, a long
   string that names this exact request. The line starts with
   `pde-gh-write: waiting for approval` and includes
   `run pde-pr-approve ID in a terminal`.
3. If the agent runs inside Herdr, a terminal program that holds many
   terminals, an approval popup opens there. A question also goes to your
   phone. Answer in either place, or run `pde-pr-approve ID` in a terminal
   to open the approval screen. The first answer counts.
4. After you approve, the waiting program runs `gh` once and passes on its
   output and exit code. After you decline, nothing is posted.
5. With no answer within 90 seconds, the program stops waiting and exits 3.
   A runner, a copy of the program started in the background with the
   request, keeps waiting and runs `gh` once you approve.
6. The agent reruns the same command. Instead of posting again, the program
   reports the result, such as `pde-gh-write: already ran ID, exit 0`,
   followed by what `gh` printed, such as a new PR's address.

## Why One Record per Request

The ID is built from the command, the title, the body, the `--repo` value as
the agent wrote it, and the directory the command runs in. The same command
with the same text from the same directory finds the same record. That is
how a rerun learns the earlier result. Changing any of these makes a new
request that needs its own answer, so an approval never covers text you did
not see or carries over to another copy of the repository.

Details the program looks up on GitHub, such as the repository name and the
PR title shown to you, are not part of the ID. If a lookup fails on a rerun,
the rerun still finds the same request.

## Why gh Runs Only Once

Only one program at a time can work on a request; the others wait their
turn. This is called holding the request's lock. Saving a request,
recording your answer, and running `gh` all happen while holding it, so if
an agent runs an approved command twice at once, the second run waits and
then finds that `gh` already ran. The runner follows the same rule, so `gh`
runs once whether the runner or a rerun gets there first.

Before starting `gh`, the program marks the record as running. A record
found still marked running means that run died partway. It is reported as
interrupted and never run again, because `gh` may already have posted.

## Why Requests Expire After Four Hours

Four hours after a record last changed, the same command becomes a new
request that needs a fresh answer. An old approval should not let a command
post hours later, when the pull request may have moved on.

Restarting the phone question or the runner does not count as a change.
Otherwise, if one keeps failing, for example because the phone tool
`moshi-hook` is missing, each rerun would restart it and push expiry back,
and the request would never expire.

## Why Body Files Are Read Once

Agents often pass long text with `--body-file PATH`. The program reads the
file when the request is made and later gives `gh` that saved text, so
editing the file after you approve cannot change what gets posted.

## How the Program Finds the PR Number

The popup and the phone name the PR a request is for, such as `merge #12`,
and its title. For `pr edit`, the approval screen also shows the
old and new title, shortened here:

```
Title:
-Old title
+New title
```

To get the title, the program asks GitHub about PR 12, so it has to pick
"12" out of the command. Commands take options such as `--title`, some of
which take a value, and agents can write them in different orders:

```
gh pr edit 12 --title "New title"             <- number first
gh pr edit --base main 12 --title "New title" <- number after other options
```

In the second command, the program knows that `main` belongs to `--base`
and `12` is the PR only because it keeps a list of the options and whether
each takes a value. Each kind of command has its own list, because the
same short option can differ: `-m` takes a milestone for `pr edit` but
nothing for `pr merge`.
When it meets an option missing from the list, it does not guess: the
popup shows `merge PR` instead of `merge #12`, and `pr edit` cannot show the
old values. The lists have to be updated when GitHub adds options to `gh`.

[The approval queue and runner](approval-queue-and-runner.md) explains the
runner. Exact states, exit codes, files, and limits are in the
[reference](../reference/requests.md).
