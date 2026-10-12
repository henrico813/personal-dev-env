# The Approval Queue and Runner

You may run several agents at once, and they can ask for approval within
seconds of each other. You often answer on the phone, sometimes long after
the agent stopped waiting. Earlier versions of the approval program went
wrong in three ways:

- Every request looked alike. The phone asked
  `Approve gh pr merge in current repository (185)?`, which does not say
  which agent asked or what PR 185 is.
- A tap meant for one request could answer the next one, which had just
  appeared under your finger.
- An approval given after the agent's 90-second wait posted nothing until
  the agent happened to rerun the command.

Two terms come up throughout. The runner is a second copy of
`pde-gh-write` that keeps waiting in the background after the agent's
command gives up, and runs `gh` once you approve. The inbox is a Herdr key,
Ctrl+b and then `a`, that reopens requests you set aside.

This page explains what you see and why it works this way. It builds on
[How a request runs gh once](request-flow.md), which explains requests,
records, and the request lock, and on
[Approving in the Herdr popup or on the phone](popup-and-phone-approval.md),
which explains the popup and the phone question and draws the popup's
screen.

## What You See

The popup and the phone question start with the same four lines; see the
[drawn popup](popup-and-phone-approval.md#what-you-see). For a comment on
PR 12 they are:

```
comment #12
octo/app
app / review login fix
Fix login timeout
```

The first four lines are the request's context:

1. What is asked, and of which PR: `comment #12`, `merge #185`, or
   `create PR`.
2. The repository. When `gh` cannot find one, for example outside a GitHub
   checkout, this line is `dir: ` and the folder name instead, so a folder
   is never mistaken for a repository.
3. Where the agent runs: the Herdr workspace and the title Herdr shows for
   the agent. When Herdr does not know the agent, for example outside
   Herdr, it is the OpenCode session ID.
4. The PR's title: the new title when the request sets one, otherwise the
   PR's current title from GitHub.

A line that cannot be found is left out rather than guessed. In the popup,
the text that will be posted comes next, as much as fits: the start of a
comment's body, or the changed lines of a `pr edit`. `v` shows the whole
request.

## Where the Context Comes From

When `pde-gh-write` makes a new request, it looks up the context and saves
it in the record:

- The repository and the PR's title come from read-only `gh` lookups,
  `gh repo view` and `gh pr view`. Each may take up to 3 seconds.
- The OpenCode session ID comes from `OPENCODE_SESSION_ID`. A small
  OpenCode plugin,
  `chezmoi/dot_config/opencode/plugins/pde-approval-session.js`, sets it for
  every command an agent runs.
- Inside Herdr, `herdr agent list` finds the agent running that session,
  which gives its pane, workspace, and title. `herdr workspace list` turns
  the workspace ID into the name you see in Herdr.

These lookups happen once and are not part of the request ID. If the
network is down when the agent reruns the command, the rerun still finds the
same request.

## The Queue: "1 of 2"

The queue is every request that is still waiting and not dismissed, oldest
first. A single popup works through it: after you answer or dismiss one
request, the popup shows the oldest one still waiting, until none are left.
The order is the time each request was made.

Each new request still asks Herdr to open the popup for it, so you do not
have to look for it. Whichever popup you answer in moves on through the
same queue.

## The Pause After a Request Appears: "one moment..."

For 600 milliseconds after each request appears, the line after `Typed:`
shows `one moment...`, and taps and Enter do nothing. Typed letters still appear
after `Typed:`.

When you answer a request, the next one is drawn at once, in the same
place. An impatient second tap would otherwise land on the new request and
answer it before you could read it. The first popup needs the pause too:
Herdr gives it the keyboard while you may be typing or tapping elsewhere.

## Tapping Twice to Confirm

In live use, accidental taps on the phone decided two requests. So the
first tap on a button only selects it: the button is drawn as
`[>> APPROVE <<]` and the line after `Typed:` says
`tap APPROVE again to confirm`. A second tap on the selected button answers. A tap on the other button
selects that one instead, and a tap anywhere else, or a typed key, clears
the selection. Typing `yes` or `no` and pressing Enter still answers in one
step, because a typed word is hard to enter by accident. Each request
starts with nothing selected, so a selection never carries over to the
next request.

Only the press of a tap counts. A tap reaches the popup as two messages,
press and release. Once the release was counted too: the press approved the
first request, the popup drew the second, and the release landed on its
APPROVE button and approved it as well.

## Dismissing, and the Inbox: Ctrl+b a

`q` or Esc dismisses a request. It keeps waiting: you can still answer on
the phone or in a terminal, and its runner keeps waiting too. But the
queue skips it, so it no longer opens on its own. Dismissing does not
restart the request's four hours.

The inbox brings dismissed requests back. Pressing Ctrl+b and then `a` in
Herdr runs `pde-pr-approve --herdr-inbox`, which opens the popup on the
oldest waiting request, dismissed or not, and then continues through the
queue. With nothing waiting, it shows a `No approvals waiting` notification
instead, so the key never seems to do nothing. The key is set in
`chezmoi/dot_config/herdr/config.toml`.

`g` closes the popup and moves you to the pane of the agent that asked, so
you can see what it is doing before you answer.

## The Sidebar Marker: "Approval waiting"

While a request waits, Herdr's sidebar shows `Approval waiting` on the pane
of the agent that asked. It goes away when the request is answered,
dismissed, or expires. Herdr also drops it after four hours, so a marker is
not left behind if the program never clears it.

One agent can ask twice. The marker belongs to the pane, not the request,
so it stays until no other waiting, undismissed request from that pane is
left.

The marker is set in the background, at the same time as the wait for your
answer, so your answer can arrive first. After setting the marker, the
program checks the record again and removes the marker if the request was
already answered.

## The Runner: Approving After the Agent Stopped Waiting

An agent's command can wait only 90 seconds, but you may answer much
later. For example:

1. 10:00:00. An agent runs `pde-gh-write pr create --title "Fix login"`.
2. 10:01:30. Nobody has answered. The command exits 3 and prints that the
   request runs once approved, so the agent should rerun the same command
   later to read the result.
3. 10:20:00. You approve on the phone. Within a second, `gh pr create`
   runs.
4. 10:25:00. The agent reruns the command. It prints
   `pde-gh-write: already ran ID, exit 0` and the new PR's address.

Step 3 is done by the runner, started as `pde-gh-write --pde-runner ID`
when the request is made. Like the phone
question, it is started apart from the agent's command, so it keeps running
when the agent's tool stops that command. It reads the record once a
second:

- When the request is approved, it runs `gh` the same way the waiting
  command would, holding the request lock. Whichever of the two gets the
  lock first runs `gh`; the other then finds that it already ran. So `gh`
  runs once.
- It saves up to 64 KiB of what `gh` printed in the record. That is how the
  rerun in step 4 can print the PR's address.
- When the request is declined or expires, it exits without running
  anything.

When it exits, it removes the sidebar marker and ends the phone question,
so a question that can no longer decide anything does not stay on your
phone.

### Which gh the Runner Uses

The runner may be started again later from the popup, a terminal, or the
phone question (see below). Their `PATH`, the list of folders searched for
programs, may not include `gh`. So the record keeps the path of the real
`gh` found when the request was made, and the runner uses that. If that
file is gone, for example after `gh` was upgraded, the runner leaves the
request approved. The agent's next rerun then runs it with the `gh` it
finds.

### Replacing a Runner That Died

A reboot or a crash can end the runner. A rerun of a waiting request, or an
approval, checks the runner's process ID in the record and starts a new
runner if the old one is gone. Without this, a late approval would never
run. Saving the new process ID does not count as a change, so it does not
restart the request's four hours.

## Ending the Phone Question

An answer in the popup or a terminal ends the phone question, so it does
not stay on your phone for up to four hours. A phone question that records
its own answer just exits.

To find out why a request did not run, see
[Debug a stuck or unanswered request](../how-to/debug-a-stuck-request.md).
Exact commands, exit codes, record fields, and limits are in the
[reference](../reference/requests.md).
