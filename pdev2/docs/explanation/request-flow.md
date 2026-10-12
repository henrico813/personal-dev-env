# How a Request Runs gh Once

Coding agents post to your pull requests: they open them, comment, change
titles, and merge. A duplicate or unwanted post is visible to everyone and
has to be cleaned up by hand. Agents also rerun a command to find out what
happened to it. This program makes each post wait for your approval and
happen only once, however often the agent reruns the command.

`gh` is GitHub's command-line tool. The guard, a short script installed as
`~/.local/bin/gh`, stops agents from using it to write to a pull request,
so they run `pde-gh-write` with the same arguments instead; see
[the guard](../reference/requests.md#the-guard).

This check runs on your computer, not as a permission rule in the tool
that runs the agent, such as OpenCode or Claude Code. Each tool has its
own rules, and people often set them to allow every command. The check
catches careless mistakes; an agent set on getting around it can find
another copy of `gh`.

One request goes like this:

1. An agent runs a pull-request write, such as `gh pr comment 12`. Your
   `.zshrc` puts `~/.local/bin` ahead of aqua's folder on `PATH`, so the
   `gh` it finds is the guard, a short bash script.
2. The guard refuses the write, exits 2, and tells the agent to use
   `pde-gh-write`. Any other command passes through to the real `gh`.
3. The agent runs the same command through `pde-gh-write`, the Go program
   built from `pdev2/`. It saves the request in a file and starts the
   runner, a copy of itself that waits in the background.
4. If the agent runs inside Herdr, `pde-gh-write` asks Herdr, through
   Herdr's socket, to open the approval plugin's popup. The popup runs
   `pde-pr-approve --herdr-popup`; `pde-pr-approve` is another name for
   `pde-gh-write`.
5. It also starts `pde-gh-write --moshi-ask`, which runs `moshi-hook ask`
   to put the question on your phone.
6. You answer in the popup, on the phone, or with `pde-pr-approve ID` in
   a terminal. The first answer counts.
7. After an approval, the agent's waiting command or the runner, whichever
   gets there first, runs the real `gh` directly, once. After 90 seconds
   the agent's command stops waiting, and only the runner is left.
8. A rerun of the same command prints the saved result instead of posting
   again.

## How It Gets Installed

`pde-installer install full` puts the pieces in place in two ways. It
builds the Go program from `pdev2/` as `~/.local/bin/pde-gh-write` and
links `pde-pr-approve` to it; the program picks its mode from the name it
was started under, so one build serves both. It also installs
`moshi-hook`. chezmoi, which the installer also runs, places the files
that tie the program into your tools: the guard as `~/.local/bin/gh`, the
Herdr plugin, the `prefix+a` inbox key, and an OpenCode plugin that tells
`pde-gh-write` which session asked. Herdr loads plugins only into a
running server, so a separate script links the plugin after each install
and, when Herdr is not running, prints how to link it later. The
terminal profile installs none of this, and removes the guard,
`pde-gh-write`, and `pde-pr-approve` if a full install left them.
[Set up approval](../how-to/set-up-approval.md) lists the steps.

The sections below explain why each step works the way it does.

## Why One Record per Request

The ID is built from the command, the title, the body, and the directory
the command runs in. The same command with the same text from the same
directory finds the same record. That is how a rerun learns the earlier
result. Changing any of these makes a new request that needs its own
answer, so an approval never covers text you did not see or carries over
to another copy of the repository.

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

## Why the Agent's Command Waits Only 90 Seconds

The tools that run agent commands, such as OpenCode, stop a command after
about two minutes, so the wait has to end first. You often answer on the
phone much later. Without something left waiting, an approval given after
the 90 seconds would post nothing until the agent happened to rerun the
command.

## The Runner

The runner is a second copy of `pde-gh-write`, started with the request as
`pde-gh-write --pde-runner ID`. It is started apart from the agent's
command, so it keeps running when the agent's tool stops that command. It
checks the record until the request is answered or expires. When the
request is approved, it runs `gh` the same way the waiting command would,
holding the lock, in the directory where the request was made. It saves
what `gh` printed in the record, so the agent's next rerun can print it.

### Which gh the Runner Uses

The runner may be started again later from the popup, a terminal, or the
phone question. Their `PATH`, the list of folders searched for programs,
may not include `gh`. So the record keeps the path of the real `gh` found
when the request was made, and the runner uses that. If that file is gone,
for example after `gh` was upgraded, the runner leaves the request
approved. The agent's next rerun then runs it with the `gh` it finds.

### Replacing a Runner That Died

A reboot or a crash can end the runner. A rerun of a waiting request, or an
approval, checks the runner's process ID in the record and starts a new
runner if the old one is gone. Without this, a late approval would wait
until the agent reran the command.

## Why the Phone Question Is Its Own Program

When the agent's command ends, the tool that ran it stops every program the
command started. You may answer the phone hours later, so `pde-gh-write`
starts the phone question as a separate program that the agent's tool does
not stop, and does not wait for it. That program is
`pde-gh-write --moshi-ask`. It runs `moshi-hook` for up to four hours, the
same as the request's lifetime.

When you answer on the phone, it saves your answer if the request is still
waiting. It never runs `gh`; the runner, the agent's waiting command, or
its next rerun does that. An answer in the popup or a terminal, or the
request expiring, ends the phone question, so a question that can no
longer decide anything does not stay on your phone.

### Why a Rerun Does Not Send a Second Question

Agents rerun a command to find out what happened to it. The request's
record keeps the process ID of the phone question, the number the system
gives a running program. A rerun of a waiting request checks whether that
program is still running:

- If it is, the rerun sends nothing, so the phone never shows two questions
  for one request.
- If it ended without an answer, for example because `moshi-hook` is
  missing, the rerun sends a new question. Otherwise you could not answer on
  the phone at all.

When the phone question ends, it removes its process ID from the record.
The system reuses process IDs, so an old one could later belong to an
unrelated program. That program would look like a running question and stop
reruns from sending a new one.

## Why Requests Expire After Four Hours

Four hours after a record last changed, the same command becomes a new
request that needs a fresh answer. An old approval should not let a command
post hours later, when the pull request may have moved on.

Saving the process ID of a new phone question or runner, or dismissing a
request in the popup, does not count as a change. A question or runner
that keeps failing is replaced on every rerun; if that counted, every
rerun would push expiry back and the request would never expire. For the
same reason, dismissing and reopening a request cannot keep it waiting
longer.

## Why Body Files Are Read Once

Agents often pass long text with `--body-file PATH`. The program reads the
file when the request is made and later gives `gh` that saved text, so
editing the file after you approve cannot change what gets posted.

## Why an Unknown Option Hides the PR Number

To name the PR, the program picks its number out of the command, and an
option it does not know might take that number as its value, so it shows
`merge PR` instead of guessing; the comments on `targetFlags` in
`internal/approval/request.go` explain the lists it uses.

Exact states, exit codes, files, and limits are in the
[reference](../reference/requests.md).
