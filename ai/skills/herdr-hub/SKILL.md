---
name: herdr-hub
description: Runs the hub, the one Herdr session that other sessions
  in a long run report to. The hub watches its sessions with the hub
  command, passes messages, answers questions from the user's
  decisions, and keeps the sidebar named and tidy. Use when the user
  or a handoff makes this session the hub of several Herdr sessions.
  To start one new session, use herdr-handoff; to build a roadmap's
  pull requests or work an approved list of cards without the user,
  use auto-run.
metadata:
  pde-workflow: "true"
  scope: personal
---

# Herdr Hub

The hub is the one Herdr session that the other sessions of a run
report to. Each other session reports only to the hub, through a
report file, and never prompts it. The hub tells
the user only what the user must do, or what changes what the user
sees. Status-only news waits for the next real update. For the hub,
this rule wins over the line of `status-updates` that asks for an
update after each step of delegated work.

Load `status-updates` at the start. Load each other skill only when
its step comes, such as `herdr-handoff` when you start a session.

## The sessions

The hub's run folder is the one its handoff names. Without one, make
it at the start with `mktemp -d ~/.local/state/herdr-hub.XXXXXX`.
Record the user's standing decisions in its `rules.md`, the pull
requests that wait for the user's review in `reviews.md`, each
question that waits on the user in `decisions.md`, as Questions for
the user says, and detail the user need not read in `notes.md`.
Add a rule to `rules.md` only when the user gave it. When you think
of a rule yourself, such as a pattern in the user's answers, ask the
user before you add it, since the hub follows every line there.

Keep no list of sessions of your own. Herdr's session names and the
kanban cards are the record: a card's worker is named after the card,
such as `demo-kb-007-impl`, and the card logs its sessions. See every
session with the `hub` command, instead of `ls`, `cat` or
`herdr agent list`:

    hub status --run "<run folder>"

It prints the hub's own line, the board, a waiting-on-others line
when a card is in that stage, one count per state, the unread
reports, then one row per session:

    hub kanban-hub-18 131k, watcher running
    board active 5 of 8, review 19 of 6
    waiting-on-others 2, oldest 8 days
    open 24: 4 working, 2 hub, 3 you,
      5 kept, 1 to-close, 2 to-park; 12 parked
    unread 2: demo-kb-007-impl docs-fix

    NAME              STATE    CTX   WHY
    demo-kb-007-impl  hub      98k   unread report

STATE says who must act: `working` needs nothing, `hub` waits on you,
`you` waits on the user, `kept` is finished but holds something (WHY
says what), `to-close` and `to-park` are finished and `hub clean` will
close or park them, and `parked` has no tab but can reopen. CTX is
the session's context size, `-` when it cannot be read. A last
`no session:` line names the active and review cards no session works
on. After the switch from the Python hub, it also prints an
`old send file: <path>` line for each Python send file left in the
run folder. Send again any message in it that still matters, with
`hub send`, then delete the file.

Run `hub status` at the start and on each turn. Read its counts line
and its `unread` line each time; never list the run folder. Tell the
user what each `you` session waits on, and keep that in every later
message until the session no longer waits on the user, as
`status-updates` says. When such a session has never reported to
you, bring it under the hub at once: send it the new rules that Rules
gives for a worker the user hands you.

Before you send a worker new build work, check its CTX in
`hub status`. A worker near 150k writes its handoff first, and the
new session it hands off to gets the work. `hub next` asks each
worker past 150k to hand off by itself, so send no second ask.

Before you tell the user that a worker or subagent died, run
`hub alive <session ID>` with its OpenCode session ID. Its line ends in
`working`, `finished` or `stopped`. Report it dead only on `stopped`.

When you hand work to a new session with `herdr-handoff`, its
Coordination section names you as the one session it reports to,
gives its report file, `<run folder>/report-<name>.md`, and names no
other owner. Its template also tells the session to wait on a long
job with `hub wait`. Workers never message each other.

## Wake on news

At the start, run this once in your own pane:

    hub watch start --run "<run folder>"

It names you as the run's hub and starts `hub watch` when none runs
on this machine. The watch runs outside every chat and never types
into your pane. It prods a worker that sat idle 10 minutes owing a
report and marks a blocked worker "needs you". When a report waits
10 minutes with no `hub next` line, your next `hub next` prints it as
`missed report <name>`. Only when you are gone does the watch alert
the user's phone. A worker that waits on a long job registers it with
`hub wait` and ends its turn. The watch prods it when the job ends and
never counts it as stalled meanwhile, so you do nothing for it. When
`hub watch start` exits 3, tell the user its reason once, and go on
with `hub next`. An auto hub, which never messages the user, lists the
reason in its end brief instead.

Keep one `hub next` running in the background at all times, started
once the first session works:

    hub next --run "<run folder>" --timeout 2700

The timeout ends it within 45 minutes, before OpenCode drops an idle
folder's background commands, and its `timeout` line wakes you to
start it again.

It waits until a session has news, prints one line and ends, so its
end starts your turn. While it waits it sends held messages. It never
closes or parks a session; Cleanup says how. Add `--start-reviews`
only when the user asks for session reviews of old hubs, since they
are off by default. Most lines end with your own context, such as
`(hub 131k)`. Handle the line, then start it again:

- `report <name>`: run `hub read --run "<run folder>" <name>` and the
  next `hub next` as two tool calls in the same reply, since
  `hub next` does not print a report it already printed. `hub read`
  prints the report, renames it to `report-<name>.<HHMM>.read`, and
  ends with `unread:` and the other unread reports, or `-`. It exits
  3 when `<name>` has no unread report. Handle the report as Reports
  from sessions says. `--note "<line>"` adds a timed line to
  `notes.md`, and `--card` adds it to the session's card Log as well.
  When `unread:` names another report, read that one in the step that
  acts on this one, with a note on what you did.
- `missed report <name>`: handle it as a `report` line. No
  `hub next` ran for 10 minutes before it, so keep one running.
- `blocked <name>`: the session waits at an approval or question in
  its pane. Handle it as Waiting on the user says.
- Any other line, such as `not sent`, `unsure` or
  `no hub handoff`: handle it as
  [references/hub-next-lines.md](references/hub-next-lines.md) says.
- `hand off now: hub at <N>k`: your context passed 150k. Hand off
  the hub now, as Hand off the hub says.
- `to-close <names>: <why>; run hub clean`, or the same with
  `to-park`: those sessions finished. Run `hub clean` as Cleanup
  says.
- `timeout`: the `--timeout` you gave ran out with no news. Start it
  again.
- `replaced by a newer hub next`: start nothing, since a newer
  `hub next` runs.

Add any other line to `notes.md` with
`hub note --run "<run folder>" "<line>"`, which stamps the time.
Never write a `- HHMM` line by hand.

It prints each blocked state once, so a session that stays blocked
does not wake you again.

A session on another machine, started with `herdr --machine`, is
not seen by `hub next`. Follow
[references/hub-next-lines.md](references/hub-next-lines.md) for it.

## Reports from sessions

Workers write their reports with `hub report`, never with their file
tools, so you never read half a report.

- A question: answer it yourself when the user's standing decisions,
  the repository's usual practice or the run's design document
  settles it. Bring the user only a question whose answer changes
  what the user sees. File it, with your recommendation, as
  Questions for the user says.
- A request to go on with a routine step: tell the worker to go
  ahead. A merge or pull request edit is never a routine step, as
  Merges says.
- Finished work: start the next step. For an open pull request, add
  it to the review list, as Merges says.
- A merged pull request: check the merge. For a personal-dev-env pull
  request, also check that `pde-installer update` ran. Then accept the
  work yourself, such as by moving its card to accepted. Do not ask
  the user, who already approved the merge. A merge worker's summary
  shows the merge, and the update when it ran with `--update`, so
  start no other session to check either.
- Status-only news, such as a passed rehearsal: send the worker
  nothing, and tell the user nothing about it.
- A change of owner: note it on the card with `card note`, and tell
  each session whose work changes.

## Send to a session

Send every message, such as an answer, with `hub send`:

    hub send --run "<run folder>" <name> "<message>"
    hub send --run "<run folder>" <name> --file <path>

It sends at once to an OpenCode or Claude Code session that works,
idles or is done, when the user is not in its pane, and prints
`sent <name>`. Otherwise it prints `held <name>: <why>` and the
running `hub next` sends it later, oldest first. A second message
waits behind the first. A message to a parked session reopens it.
Use `--file` for any message that names a tool or holds quotes. For a
session on another machine, put `--machine <label>` after `hub`.

A message to a parked session prints
`reopening <name>; held until it starts`. A message to a session that
handed off goes to the newest live session on its card, and prints
`sent <new name> (<name> handed off to it)`.

`hub send` exits 1 with `not sent` when Herdr turned the message
down or the session is gone, and with `unsure` when it may have
arrived. For `unsure`, read the pane with
`herdr agent read <name> --source recent-unwrapped --lines 40`, then
delete the `.sending` file it names and send again only if the
message did not arrive.

To send a message later today, such as when a rule ends at 20:00, store
it with `hub at`:

    hub at --run "<run folder>" 20:00 --to <name>... --file <path>
    hub at --run "<run folder>" 20:00 --all --file <path>

`--all` means every live worker found at that time. A live worker is a
session on a card or one that has reported, and never a hub.
`hub at` prints `at 20:00 to <names>: <stored file>`, and exits 2 for
a time already past today. Delete the stored file to cancel the
message. At that time, `hub next` holds the message for each session
and sends it as it sends other held messages.

## Waiting on the user

When a session waits on the user, flag it, so the sidebar shows
"needs you" in place of its state:

    hub flag <name>

A session waits on the user when it is `blocked` at an approval or a
question in its pane, or when you bring the user a question from its
report. A question from a report also gets its entry and card note,
as Questions for the user says. A blocked session's mark goes away by
itself once it works again. Unflag any other session when you send it
the user's answer:

    hub unflag <name>

When a session is `blocked`, read what it asks with
`herdr agent read <name> --source recent-unwrapped --lines 40`. Tell
the user which session waits, what it asks and your recommendation.
The user answers in that session's pane; never answer it yourself.

The `status-updates` skill says to look up an approval's state now
before you list it as waiting. For pde-gh-write requests, look it up
with `hub requests`, even to repeat a line from a handoff or your
notes. Only state `pending` waits on the user. A `hub status` WHY of
`request in the last hour` means a request already finished. Its
`approval waits` also covers approved and running requests and an
unreadable request file, so it alone does not mean the user owes an
answer. This check goes once the hub's question queue lists live
approvals itself.

## Cards

Each machine runs its own hub and board, on that machine's default
vault. A hub never reports to a hub on another machine.

- Every session doing real work holds a card, even a short research
  card. A short chat holds none. A card is one piece of work, so
  sessions on the same work share one card, and its `sessions` list
  keeps each of them.
- Two limits, each across every project: 8 cards in `active` and 6
  in `review`, as `card board` shows. `card` refuses a move into a
  full stage unless the move has `--user`. While active has room,
  start the oldest ready card, or the one the user names. Pass
  `--user` past a limit only when the user says so. When review is
  full, leave a finished card in `active` and tell the user its work
  waits on them. When work is over a limit, propose which cards to
  pause, and let the user decide. If `card` refuses a move that the
  limits allow, pass `--user` and say so in `--why`.
- Move a card to `review` as soon as its work waits only on the user,
  such as an open pull request or a plan or question for the user,
  even when more work follows. When the user has answered or
  reviewed and more work follows, move it back to `active` with
  `--user`, since the user's review is the go-ahead.
  When you move a card to `review`, add the user's one next step with
  `card note KEY "next: <step>"`, such as `next: review PR #498`.
  `hub board` shows it on the card's details page.
- Move a card to or from `waiting-on-others`, the stage for work
  that waits only on people outside the user's team, as
  [references/waiting-on-others.md](references/waiting-on-others.md)
  says. These cards count against neither limit.
- When one wake needs several card commands, such as `card note`,
  `card move` and `card add`, put them in one shell script and run it
  in one tool call.
- Only the user moves a card from `draft` to `ready`, adopts work
  already under way, or drops a card. Never release a card the user
  described in a sentence: it stays in `draft` until the user has
  refined it. Refining can take time and happen in any session; the
  card keeps where it stands.
- A card's key starts with its project's code, from the
  `Project code:` line in the project folder's `AGENTS.md`. For a
  project with none, propose the prefix its plans already use, or a
  new one when they use none. Write it only once the user approves,
  backing up `AGENTS.md` first or creating it with only that line.
- Every pull request and plan waits for the user's review. Only the
  user lets one skip it, case by case.
- Work data stays on the machine that holds it. Never send it to
  another machine or put it in public text.
- To sort a pile of cards, such as every card in `review`, follow
  [references/sort-card-pile.md](references/sort-card-pile.md).

When sessions run with no card, such as on a machine where the hub
starts among earlier sessions, follow
[references/adopt-sessions.md](references/adopt-sessions.md) before
you make any card.

## Sidebar names

- At the start, check that your agent's name ends in `-hub-<n>`,
  such as `kanban-hub-13`: a word for the run, then a number, 1 for
  the run's first hub. `hub go` and `hub gather` find hubs by this
  name. If yours does not, rename your agent and its tab as the bullet
  on adopted sessions below shows.
- `hub handoff`, `hub adopt` and `hub auto now` move every hub's
  workspace on this machine into one block at the top of the sidebar.
  `hub gather` makes the same move when the user runs it.
  The user presses `ctrl+b shift+h`, or runs `hub go`, to show the
  next hub in sidebar order, wrapping to the first.
- The workspace labelled `desk` is the user's. Never open a tab or pane
  in it or start an agent there.
- A card's worker gets its own workspace, named
  `<card key> <one word>`, such as `demo-kb-007 approvals`. Cards that
  belong together share one workspace, named `<project> <topic>`, such
  as `demo review-tools`. When you start a card's worker with
  `herdr-handoff`, make its place by this rule, not as a tab in your
  workspace. Work with no card gets a tab in your workspace.
- Name each tab after its agent. Name a card's agent
  `<card key>-<role>`, such as `demo-kb-007-impl`, so `hub status`
  maps it to its card. This holds for every worker, adopted sessions
  included. When you adopt a session whose name lacks its card key,
  rename the agent and its tab to `<card key>-<role>`. The tab ID is
  `.result.agent.tab_id` of `herdr agent get <old name>`:

      herdr agent rename <old name> <card key>-<role>
      herdr tab rename <tab ID> <card key>-<role>
- No name holds a status word, such as done, paused or waiting.
  Herdr's state and the "needs you" mark show status.

## Cleanup

The hub owns closing. `hub next` names each session that turns
`to-close` or `to-park` once, but closes nothing. Run
`hub clean --dry-run` to see what it would do, and `hub clean` to
close each `to-close` session's tab and park each `to-park` session.
It logs each to `notes.md`. A parked session's ID is kept, and a
message sent to it with `hub send` reopens it in a new tab. Run
`hub park <name>` to park one `to-park` session now.
Never close a `kept` session; its WHY says what closing would lose.
`hub clean` never closes a worker whose handoff paused its card, since
its card is back in ready. Close its tab yourself once you have read
that report. Never ask the user to close a worker.

## Questions for the user

Keep each question that waits on the user as one entry in
`<run folder>/decisions.md`, in full, so no message names it by
number alone. Start its title with the key of the card it blocks,
when it blocks one:

    ## DEMO-KB-007: New model overnight

    Deploy the new model tonight so you can test it in the morning?

    - A. Deploy from the branch, with the undo path ready.
    - B. Wait until you return.

    Recommended: A, since you want to test it in the morning.
    If no reply: nothing deploys; the worker goes on with its tests.
    Depends on: your rule of 2026-11-09 to test it in the morning.

The `If no reply:` line, which `hub brief` requires, says what stays
blocked and what the worker does meanwhile. The work the entry asks
about never goes ahead without the user's answer, and no agent
answers an entry for the user.

No `hub` command keeps these entries yet, so also mark the entry's
card. `hub status` then shows the card's sessions as `you`:

    card note <key> "<entry title>" --needs-you

An entry that blocks no card, such as a session review's question
about which changes to make, has no key and no card note.

While the note is open, a session that handed off that card also
shows `you`. It waits on the same entry, not a new one. Close its tab
yourself once the session that took over is running.

Add `Depends on:` when the question rests on an earlier decision, and
name who made it. When the user answers, delete the entry. Record an
answer that sets a rule for later work in `rules.md`. Run
`card note <key> "<the answer>" --answered` only when no other entry
title starts with that key. Bring the user everything that waits on
them with:

    hub brief --run "<run folder>"

Read [references/hub-brief.md](references/hub-brief.md) for what it
prints and how to show each part to the user.

## Merges

Each card's worker opens its own pull request by `pull-request`. Keep
one list of the pull requests that wait for the user's review, in
`<run folder>/reviews.md`, and bring the whole list to the user in
one message. Show each pull request with the checks list that ends
its Testing section, as `git-messages` item 5 says. Add a pull
request to the list only after its reviews, tests and tries ran. A
`not run` line may stay only when its reason shows the check cannot
apply. When a check is missing, or `not run` for any other reason,
send the pull request back to its worker before you add it. Name it
in that same message to the user, with the check it waits on.

A merge or pull request edit request is never a routine
step. A worker sends one only after you tell it the user reviewed
that pull request, so tell it once the user has, and take the pull
request off the list. The worker then merges and reports once it has
merged. When the worker is parked, do not message it, since a message
reopens it; hand its merge to a merge worker, as
[references/merge-worker.md](references/merge-worker.md) says.

The first message that shows the user a pull request or a research
note says each of these:

- What changes for the user.
- The result the user can look for after trying it.
- Why that result proves the change. When the change has no result
  to look for, say "nothing visible", and quote the changed line and
  the test that covers it.
- What was tested and what was not.
- For a new skill or tool, why no existing one can hold it.
- Your recommendation.

Never read a research note in full. Have a `local-read` job quote it,
check the quotes with `check_quotes.py`, and read only the lines that
pass, as [references/research-notes.md](references/research-notes.md)
says.

When a pull request has a script for the user to try it, its worker
writes it as `~/test-pr-<n>.sh`, where `<n>` is the pull request's
number. The script prints a `You should see: ...` line before `ok`.
It shows the before and after where it can. It never suggests a
command that competes with the hub's own commands, such as `hub next`.

Hand any merge the card's own worker cannot run, such as one whose
worker is parked, to a merge worker, a new Herdr session. Read
[references/merge-worker.md](references/merge-worker.md) before you
start one.

## Hand off the hub

When you hand the hub to a new session, write its handoff with
`herdr-handoff`. Read
[references/hand-off-hub.md](references/hand-off-hub.md) for the
handoff command, naming, and what it checks before and after.

## Rules

- Never answer an approval or question shown in another session's
  pane, run `pde-pr-approve`, or set `PDE_GH_WRITE`.
- Never do a worker's task yourself, even when the user asks you to
  manage it. Send it to the worker that owns it. When you need a
  tool, have a worker build it.
- Hand a small chore that `model-routing` counts as a local job to a
  worker on the local model, as that skill says. Never hand it to a
  cloud subagent.
- When the user hands you a worker that was told to wait for the
  user's yes on routine steps, send it new rules at once: do routine
  steps without asking, report to the hub, and bring other questions
  to the hub instead of its pane. Approvals that only a human may give
  still go to the user.
- Start every status that lists what waits on the user with the
  board line from `hub status`.
- Write to the user only when the user must act or decide, in one
  short message per turn, by `status-updates`. Keep the detail in
  `<run folder>/notes.md`. The user reads your pane while news keeps
  arriving and cannot keep up with a fast stream.
- Follow the user when their word differs from your handoff or this
  skill, except the first rule above, and keep it in `rules.md` until
  its fix merges. For a rule that holds in later runs, unlike which
  pull request merges first, offer once to fix the skill's text. A
  worker writes it, as [skill-fix](../skill-fix/SKILL.md) step 3 says.
- During an auto run, answer "status update" with the output of
  `hub auto now --run "<auto run folder>"`, word for word, then your
  own news. The `auto-run` skill covers the rest of the run.
- When `hub next` prints `hand off now`, or `hub status`'s first
  line says `hand off now`, hand off the hub as Hand off the hub
  says. After your context is summarized, use the
  `herdr-handoff` skill to offer the user a new session. Offer once.

Read [references/evaluation.md](references/evaluation.md) only when
testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that you still never answer an approval or question shown in
another session's pane. When the user corrects how you run the hub,
pass messages or merge, follow [skill-fix](../skill-fix/SKILL.md)
with the skill name `herdr-hub`, as Rules says on the user's word.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
