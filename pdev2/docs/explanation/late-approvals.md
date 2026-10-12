# Approving After the Agent Stopped Waiting

An agent's command waits for your answer for only 90 seconds, but you often
answer on the phone much later. Before the runner, an approval given after
the wait posted nothing until the agent happened to rerun the command, and
the phone question stayed on your phone for hours after you had answered
in the popup.

The runner is a second copy of `pde-gh-write` that keeps waiting in the
background after the agent's command gives up, and runs `gh` once you
approve. This page explains what you see and why it works this way. It
builds on [How a request runs gh once](request-flow.md), which explains
requests, records, and the request lock.

## What You See

1. 10:00:00. An agent runs `pde-gh-write pr create --title "Fix login"`.
   It prints that it is waiting for approval and that the request runs once
   approved, so the agent should rerun the same command later to read the
   result.
2. 10:01:30. Nobody has answered. The command exits 3 and says it is still
   waiting.
3. 10:20:00. You approve on the phone. Within a second, `gh pr create`
   runs.
4. 10:25:00. The agent reruns the command. It prints
   `pde-gh-write: already ran ID, exit 0` and the new PR's address.

## The Runner

Step 3 is done by the runner, started as `pde-gh-write --pde-runner ID`
when the request is made. Like the phone question, it is started apart from
the agent's command, so it keeps running when the agent's tool stops that
command. It reads the record once a second:

- When the request is approved, it runs `gh` the same way the waiting
  command would, holding the request lock, in the directory where the
  request was made. Whichever of the two gets the lock first runs `gh`; the
  other then finds that it already ran. So `gh` runs once.
- It saves up to 64 KiB of what `gh` printed in the record. That is how the
  rerun in step 4 can print the PR's address. The waiting command saves it
  too, and prints it once `gh` exits.
- When the request is declined or expires, it exits without running
  anything.

When the request expires, it ends the phone question, so a question that
can no longer decide anything does not stay on your phone.

## Which gh the Runner Uses

The runner may be started again later from the popup, a terminal, or the
phone question (see below). Their `PATH`, the list of folders searched for
programs, may not include `gh`. So the record keeps the path of the real
`gh` found when the request was made, and the runner uses that. If that
file is gone, for example after `gh` was upgraded, the runner leaves the
request approved. The agent's next rerun then runs it with the `gh` it
finds.

## Replacing a Runner That Died

A reboot or a crash can end the runner. A rerun of a waiting request, or an
approval, checks the runner's process ID in the record and starts a new
runner if the old one is gone. Without this, a late approval would wait
until the agent reran the command. Saving the new process ID does not
count as a change, so it does not restart the request's four hours.

## Ending the Phone Question

An answer in the popup or a terminal ends the phone question, so it does
not stay on your phone for up to four hours. A phone question that records
its own answer just exits.

Exact commands, exit codes, record fields, and limits are in the
[reference](../reference/requests.md#runner).
