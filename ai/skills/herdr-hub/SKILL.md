---
name: herdr-hub
description: Runs the hub, the one Herdr session that other sessions
  in a long run report to. The hub watches its sessions with the hub
  command, passes messages, answers questions from the user's
  decisions, and keeps the sidebar named and tidy. Use when the user
  or a handoff makes this session the hub of several Herdr sessions.
  To start one new session, use herdr-handoff; to build a roadmap's
  pull requests, use roadmap-run; to work an approved list of cards
  without the user, use auto-run.
metadata:
  pde-workflow: "true"
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

Keep no list of sessions of your own. Herdr's session names and the
kanban cards are the record: a card's worker is named after the card,
such as `demo-kb-007-impl`, and the card logs its sessions. See every
session with the `hub` command, instead of `ls`, `cat` or
`herdr agent list`:

    hub status --run "<run folder>"

It prints the hub's own line, the board, one count per state, the
unread reports, then one row per session:

    hub kanban-hub-18 131k, watcher running
    board 24/8, review 19
    open 24: 4 working, 2 hub, 3 you,
      5 kept, 1 to-close, 2 to-park; 12 parked
    unread 2: demo-kb-007-impl docs-fix

    NAME              STATE    CTX   WHY
    demo-kb-007-impl  hub      98k   unread report

STATE says who must act: `working` needs nothing, `hub` waits on you,
`you` waits on the user, `kept` is finished but holds something (WHY
says what), `to-close` and `to-park` are finished and `hub next` will
close or park them, and `parked` has no tab but can reopen. CTX is
the session's context size, `-` when it cannot be read. A last
`no session:` line names the active and review cards no session works
on. After the switch from the Python hub, it also prints an
`old send file: <path>` line for each Python send file left in the
run folder. Send again any message in it that still matters, with
`hub send`, then delete the file.

Run `hub status` at the start and on each turn. Read its counts line
and its `unread` line each time; never list the run folder. Tell the
user once what each `you` session waits on. When such a session has
never reported to you, bring it under the hub at once: send it the
new rules that Rules gives for a worker the user hands you.

Before you send a worker new build work, check its CTX in
`hub status`. A worker near 150k writes its handoff first, and the
new session it hands off to gets the work. `hub next` asks each
worker past 150k to hand off by itself, so send no second ask.

Before you tell the user that a worker or subagent died, run
`hub alive <session ID>` with its OpenCode session ID. Its line ends
in `working`, `finished` or `stopped`. Report it dead only on
`stopped`.

When you hand work to a new session with `herdr-handoff`, its
Coordination section names you as the one session it reports to,
gives its report file, `<run folder>/report-<name>.md`, and names no
other owner. Workers never message each other.

## Wake on news

Keep one `hub next` running in the background at all times, started
once the first session works:

    hub next --run "<run folder>"

It waits until a session has news, prints one line and ends, so its
end starts your turn. While it waits it sends held messages, and it
closes and parks finished sessions as Cleanup says. Most lines end
with your own context, such as `(hub 131k)`. Handle the line, then
start it again:

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
- `blocked <name>`: the session waits at an approval or question in
  its pane. Handle it as Waiting on the user says.
- `not sent <name>: <reason>`: Herdr turned a held message down, or
  the session is gone. The message stays in a `.notsent` file in
  `<run folder>/sends/`. Check the session, and send it again. Once
  you have sent it again, or decided not to, delete that `.notsent`
  file, since the session waits on you while it stays.
- `unsure <name>: <reason>`: a held message may have arrived. Handle
  it as an `unsure` send, as Send to a session says.
- `hand off now: hub at <N>k`: your context passed 150k. Hand off
  the hub now, as Hand off the hub says.
- `board over its limit: N/M`: the cards in active and review passed
  the board's limit. Tell the user at once, with the board line, and
  start no new work until the count is back under the limit or the
  user says to go on.
- `replaced by a newer hub next`: start nothing, since a newer
  `hub next` runs.

Add any other line to `notes.md` with
`hub note --run "<run folder>" "<line>"`, which stamps the time.
Never write a `- HHMM` line by hand.

It prints each blocked state once, so a session that stays blocked
does not wake you again.

A session started with `herdr --machine <label>` runs on that
machine's Herdr server, and you cannot read its files, so `hub next`
does not see it. Write in its handoff that it ends each turn with its
report. Learn of the turn's end with one background command, and read
the report with the second:

    herdr --machine <label> agent wait <name> --until idle \
      --until done --until blocked
    herdr --machine <label> agent read <name> \
      --source recent-unwrapped --lines 80

`hub --machine <label> status --run "<run folder>"` lists that
machine's sessions.

## Reports from sessions

Workers write their reports with `hub report`, never with their file
tools, so you never read half a report.

- A question: answer it yourself when the user's standing decisions,
  the repository's usual practice or the run's design document
  settles it. Bring the user only a question whose answer changes
  what the user sees, with your recommendation.
- A request to go on with a routine step: tell the worker to go
  ahead. A merge or pull request edit is never a routine step, as
  Merges says.
- Finished work: start the next step. For an open pull request, add
  it to the review list, as Merges says.
- A merged pull request: check the merge, and its install where there
  is one, then accept the work yourself, such as by moving its card to
  accepted. Do not ask the user, who already approved the merge.
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
arrived. For `unsure`, read
the pane with
`herdr agent read <name> --source recent-unwrapped --lines 40`, then
delete the `.sending` file it names and send again only if the
message did not arrive.

## Waiting on the user

When a session waits on the user, flag it, so the sidebar shows
"needs you" in place of its state:

    hub flag <name>

A session waits on the user when it is `blocked` at an approval or a
question in its pane, or when you bring the user a question from its
report. A blocked session's mark goes away by itself once it works
again. Unflag any other session when you send it the user's
answer:

    hub unflag <name>

When a session is `blocked`, read what it asks with
`herdr agent read <name> --source recent-unwrapped --lines 40`. Tell
the user once which session waits, what it asks, and your
recommendation. Never answer it yourself.

## Cards

Each machine runs its own hub and board, on that machine's default
vault. A hub never reports to a hub on another machine.

- Every session doing real work holds a card, even a short research
  card. A short chat holds none. A card is one piece of work, so
  sessions on the same work share one card, and its `sessions` list
  keeps each of them.
- One limit, across every project: 8 cards in `active` and
  `review` together, as `card board` shows. While it allows, start
  the oldest ready card, or the one the user names. Never start one
  past the limit; only the user does. When work is over the limit,
  propose which cards to pause, and let the user decide. If `card`
  refuses a start that the limit allows, pass `--user` and say so in
  `--why`.
- Move a card to `review` as soon as its work waits only on the user,
  such as an open pull request or a plan or question for the user,
  even when more work follows. When the user has answered or
  reviewed and more work follows, move it back to `active` with
  `--user`, since the user's review is the go-ahead.
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
- Then run `hub gather`, which takes no `--run`. It moves every hub's
  workspace on this machine into one block at the top of the sidebar.
  The user presses `ctrl+b shift+h`, or runs `hub go`, to show the
  next hub in sidebar order, wrapping to the first.
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

The hub owns closing. `hub next` closes each `to-close` session's tab
and parks each `to-park` session by itself, at most once a minute,
and logs each to `notes.md`. A parked session's ID is kept, and a
message sent to it with `hub send` reopens it in a new tab. Run
`hub clean --dry-run` to see what it would do, and `hub clean` to do
it now. Run `hub park <name>` to park one `to-park` session now.
Never close a `kept` session; its WHY says what closing would lose.
`hub next` never closes a worker whose handoff paused its card, since
its card is back in ready. Close its tab yourself once you have read
that report. Never ask the user to close a worker.

## Questions for the user

Keep each question that waits on the user as one entry in
`<run folder>/decisions.md`, in full, so no message names it by
number alone:

    ## New model overnight

    Deploy the new model tonight so you can test it in the morning?

    - A. Deploy from the branch, with the undo path ready.
    - B. Wait until you return.

    Recommended: A, since you want to test it in the morning.
    If no reply: A.
    Depends on: your rule of 2026-11-09 to test it in the morning.

Add `Depends on:` when the question rests on an earlier decision, and
name who made it. Whenever a message to the user mentions an entry,
show the entry in full. Delete the entry once the user answers, and
record the answer in `rules.md` when it stands for later. Bring the
user everything that waits on them with:

    hub brief --run "<run folder>"

Its first line is `board N/M, review R; K decisions`, then it prints
`reviews.md` and every entry. It exits 3, naming the entry, when one
lacks its question, two choices, `Recommended:` or `If no reply:`.
Fix the entry and run it again.

## Merges

Each card's worker opens its own pull request by `pull-request`. Keep
one list of the pull requests that wait for the user's review, in
`<run folder>/reviews.md`, and bring the whole list to the user in
one message. A merge or pull request edit request is never a routine
step. A worker sends one only after you tell it the user reviewed
that pull request, so tell it once the user has, and take the pull
request off the list. The worker then merges and reports once it has
merged. When the worker is parked, do not message it, since a message
reopens it; hand its merge to a merge worker, as below.

Hand any other merge first to a merge worker: a new Herdr session on
the local model, started by `herdr-handoff` with
`--model goog/qwen3.8 --auto`, never a subagent of the hub. A
subagent ends when the hub's turn is interrupted, and the user cannot
see it. Make a folder with `mktemp -d`. The worker's handoff file
holds only this brief, filled in, and none of the handoff template's
sections. In a repository with several remotes, add
`--remote <remote>` after the test command.

    From <repository>, run this one command once:
    bash ~/.agents/skills/herdr-hub/scripts/merge-worker.sh \
      <number> <folder> '<test command>'
    Then print <folder>/summary.txt exactly and stop. Never run it
    again, even when a check fails. Touch no other file and run
    nothing else. Never answer an approval request.

Read `<folder>/summary.txt` yourself. It counts only when its last
line is `done`; never trust the worker's words about it. When it
says `result: ready`, start a cloud session by `herdr-handoff` that
follows `pull-request`'s Merge from step 4, with `<folder>/prep` as
`$O`. Otherwise bring the problems it names to the user with the
pull request's review entry. Close the merge worker's tab once you
have read its summary.

## Hand off the hub

When you hand the hub to a new session, write its handoff with
`herdr-handoff`, holding only the run's state, then start it with:

    hub handoff --run "<run folder>" <name> <handoff file>

Name it as Sidebar names says, with `<n>` one higher than yours, such
as `kanban-hub-14` after `kanban-hub-13`.

It starts `<name>` in a tab of your workspace on the model of the last
`--model '...'` line in `rules.md`, so keep the hub start command there
in that form. The new hub's tab gets `HUB_RUN` set to the run folder.
It checks its inputs before it starts anything. Before it
sends the handoff, it writes your session review's handoff and adds to
the new hub's handoff how to close your tab and start that review. It
then reads back the reasoning effort of the new hub's first reply. Tell
the user the effort it printed, never one you assume. When it exits 3
with `started <name>, but`, the new hub runs: tell the user, who can
switch its effort in that session, and never run `hub handoff` again.

## Rules

- Never answer an approval or question shown in another session's
  pane, run `pde-pr-approve`, or set `PDE_GH_WRITE`.
- Never do a worker's task yourself, even when the user asks you to
  manage it. Send it to the worker that owns it. When you need a
  tool, have a worker build it.
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
- When the user's word differs from your handoff or this skill,
  follow the user and record the new rule in `rules.md`. You still
  never answer an approval or question shown in another session's
  pane.
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
pass messages or merge, follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`herdr-hub`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
