# Approving in the Herdr Popup or on the Phone

When an agent asks to post to a pull request, you do not have to switch to
a terminal and type `pde-pr-approve ID`. Two quicker ways to answer appear
on their own:

- Herdr is a terminal program that holds many terminals, and the agents run
  inside it. Herdr opens a small approval window, the popup, on top of
  them.
- Moshi is a phone app. `moshi-hook` is a command that sends a yes-or-no
  question to it and waits for your answer.

What happens before and after your answer is in
[How a request runs gh once](request-flow.md).

## What You See

An agent in the Herdr workspace `app` asks to post a four-line comment on
PR 12 while another request also waits. The popup looks like this. It is
37 columns by 14 rows, inside a border that Herdr draws:

```
comment #12                    1 of 2  <- what is asked; first of two waiting
octo/app                               <- repository
app / review login fix                 <- Herdr workspace / agent's title
Fix login timeout                      <- the PR's title
Looks good. One question:              <- the comment, as much as fits
should the timeout be 30s?
The old value was 10s.
Typed:                                 <- keys typed so far, then notices
v: full; g: focus; q/Esc: dismiss

             [ APPROVE ]               <- tap row 10 or 11 to approve


             [ DECLINE ]               <- tap row 13 or 14 to decline
```

The first four lines say which request this is and where it came from;
[The approval queue and runner](approval-queue-and-runner.md) explains them
and the `1 of 2`. The text that will be posted fills the rows below them;
`v` shows all of it. For a `pr edit` that renames the PR and changes its
body, those rows show what changes instead:

```
-Fix login
+Fix login timeout
-Old body
```

The first tap on a button only selects it. The screen then shows:

```
Typed:   tap APPROVE again to confirm
v: full; g: focus; q/Esc: dismiss

           [>> APPROVE <<]
```

A second tap on APPROVE approves. Instead of tapping, you can type `yes` or
`no` and press Enter.

At the same time, the phone shows the same first four lines as a question:

```
Approve comment #12?
octo/app
app / review login fix
Fix login timeout
```

## Three Places to Answer; the First Wins

You can answer in the popup, on the phone, or with `pde-pr-approve ID` in a
terminal. Each one saves your answer only if the request is still waiting.
It checks while holding the request's lock (see
[Why gh runs only once](request-flow.md#why-gh-runs-only-once)), so two
answers cannot both be saved.

An answer in the popup or a terminal ends the phone question, so it does
not stay on your phone for hours. A popup still showing a request answered
elsewhere stays open, and a phone question that could not be ended stays on
the phone. A later answer from either finds the request already answered or
expired and changes nothing. A tap on an old popup
cannot undo an approval after `gh` may have posted, and an old phone
question cannot approve a request you declined.

## What Stops an Accidental Answer

- **Keys typed before the popup appears are thrown away.** The program
  asks Herdr to put the keyboard on the popup when it opens, so a `yes` and
  Enter you were typing for an agent could otherwise land in the popup and
  approve.
- **One tap does not answer.** The first tap on a button only selects it,
  and taps and Enter do nothing for a moment after each request appears;
  see [The approval queue and runner](approval-queue-and-runner.md).
- **A single `y` is not an answer.** An earlier version approved on one
  stray key. Typing `y` and Enter shows `Type yes or no, then Enter`.
- **Agent text cannot change the screen.** The agent writes the title and
  body. Characters that tell a terminal to clear the screen or move the
  cursor are shown as plain text instead, so the clear-screen code
  `ESC [2J` appears as `^[[2J`. Otherwise an agent could draw a different
  request over the real one.
- **`v` shows everything.** The popup has room only for a summary. `v` opens
  the full request in `less`, the same text `pde-pr-approve` shows. Keys
  typed while `less` is open, such as `yes` typed into its search, are
  thrown away when it closes.
- **`q` or Esc dismisses the request without answering.** It keeps waiting
  for an answer elsewhere, but the popup no longer opens for it on its own.
  `Ctrl+b a` in Herdr opens it again.

## Why the Phone Question Keeps Running

The agent's command waits for an answer for only 90 seconds. When the
command ends, the tool that ran it stops every program the command started.
You may answer the phone hours later, so `pde-gh-write` starts the phone
question as a separate program that the agent's tool does not stop, and
does not wait for it. That program is `pde-gh-write --moshi-ask`. It runs
`moshi-hook` for up to four hours, the same as the request's lifetime.

When you answer on the phone, it saves your answer if the request is still
waiting. It never runs `gh`. The request's runner, the agent's waiting
command, or its next rerun of the same command does that; see
[The approval queue and runner](approval-queue-and-runner.md).

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

### Why This Does Not Extend the Four Hours

A request expires four hours after its record last changed. Saving or
removing the process ID does not count as a change. If it did, a question
that kept failing would be replaced on every rerun, each replacement would
push expiry back, and the request would never expire.

## How the Popup Is Opened

Herdr sets `HERDR_ENV=1` in the terminals it runs; without it, no popup is
opened. With it, `pde-gh-write` runs `herdr status server` to find Herdr's
socket, a file that other programs on the same computer use to send Herdr
requests. It sends one request there: open the `approval` popup of the
`pde.approval` plugin, and give it the request ID in `PDE_REQUEST_ID`. The
plugin file, which chezmoi deploys from
`chezmoi/dot_local/share/pde/herdr-plugins/pde-approval/`, tells Herdr the
popup's size and to run `pde-pr-approve --herdr-popup` in it. The `herdr`
command has no option that opens a popup, which is why the program uses the
socket.
`pde-gh-write` also marks the agent's pane with `Approval waiting` in
Herdr's sidebar and asks Herdr to show a notification with a sound.

This runs in the background and ignores errors. A slow or missing Herdr
must not delay the request, and you can still answer on the phone or in a
terminal.

Popup keys, exit codes, and sizes are in the
[reference](../reference/requests.md#herdr-popup).
