---
name: herdr-hub
description: Runs the hub, the one Herdr session that other sessions
  in a long run report to. The hub watches its sessions with the hub
  command, passes messages, answers questions from the user's
  decisions, and keeps the sidebar named and tidy. Use when the user
  or a handoff makes this session the hub of several Herdr sessions.
  To start one new session, use herdr-handoff; to build a roadmap's
  pull requests, use roadmap-run.
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

It prints one line for every named session except the hub:

    NAME              STATE  CTX    CARD         REPORT  SEND  LABEL
    demo-kb-007-impl  idle   98.3K  DEMO-KB-007  unread  -     needs you

CTX is the context size from the session's footer. REPORT is `unread`
while `report-<name>.md` waits in the run folder. SEND is `held` while
a message waits for the session, `failed` when Herdr refused one, and
`unsure` when a send stopped part way. A named session missing from
the table is gone.

Below it comes one line per active card, with the card's newest live
session:

    CARD         SESSION           STATE  WAITS
    DEMO-KB-007  demo-kb-007-impl  idle   you

WAITS is `hub` when the session waits on you, and `you` when it waits
on the user, even when it does not report to you. A session waits on
the user when:

- it is blocked, or marked "needs you"
- its card has a needs-you note with no answer
- it sits idle and has never reported to you

Run `hub status` at the start and on each turn. Tell the user once
what each `you` session waits on. When such a session has never
reported to you, bring it under the hub at once: send it the new
rules that Rules gives for a worker the user hands you.

When you hand work to a new session with `herdr-handoff`, its
Coordination section names you as the one session it reports to,
gives its report file, `<run folder>/report-<name>.md`, and names no
other owner. Workers never message each other.

## Wake on news

Keep one `hub next` running in the background at all times, started
once the first session works:

    hub next --run "<run folder>"

It waits until a session has news, prints one line and ends, so its
end starts your turn. While it waits it sends held messages. Handle
the line, then start it again:

- `report <name>`: read `<run folder>/report-<name>.md`, handle it as
  Reports from sessions says, then rename it to `report-<name>.read`.
- `blocked <name>`: the session waits at an approval or question in
  its pane. Handle it as Waiting on the user says.
- `failed <name>: <reason>`: Herdr refused a held message before
  taking it. Read the message in `<run folder>/send-<name>.failed`,
  check the session, and send it again.
- `unsure <name>: <reason>`: a held message may have arrived. Handle
  it as an `unsure` send, as Send to a session says.

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

It sends at once when the session is `idle` or `done` and the user is
not in its pane, and prints `sent <name>`. Otherwise it prints
`held <name>` and keeps the message in the run folder, and the
running `hub next` sends it when the session is free. For a session
on another machine, put `--machine <label>` after `hub`.

`hub send` exits 1 with Herdr's reason when Herdr refuses the
message. When Herdr gives up after taking it, the send shows as
`unsure`, since the message may have arrived. `hub send` refuses a
second message, with exit code 2, while one to that session is held
or `unsure`. Wait for the held one to go out.
For `unsure`, read the pane with
`herdr agent read <name> --source recent-unwrapped --lines 40` to see
whether the message arrived. Then delete
`<run folder>/send-<name>.sending` and send again only if it did not.

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
- Two limits, across every project: 8 cards in `active` and 6 in
  `review`. While both allow, start the oldest ready card, or the one
  the user names. Never start one past a limit; only the user does.
  When active work is over its limit, propose which cards to pause,
  and let the user decide. If `card` refuses a start that both limits
  allow, pass `--user` and say so in `--why`.
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

- Each worker closes its own tab when its card is accepted or
  dropped, or once it has written a handoff that pauses its card. It
  first checks that it holds nothing unsaved, such as uncommitted files
  or an unpushed branch. A paused worker may leave such files, since
  closing a tab changes no file, but its handoff lists each one. The
  handoff that starts it says so, and you tell it when its card is
  accepted or dropped.
- When a worker cannot close its tab, close it yourself, once
  `hub status` shows it `idle` or `done` and it holds nothing unsaved,
  or, for a pause, its handoff lists each file it leaves. Never ask
  the user to close a worker.
- A cleanup worker sweeps the tabs left open with the session sweep
  script, once that script exists.

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

It prints `reviews.md`, then every entry. It exits 2, naming the
entry, when one lacks its question, two choices, `Recommended:` or
`If no reply:`. Fix the entry and run it again.

## Merges

Each card's worker opens its own pull request by `pull-request`. Keep
one list of the pull requests that wait for the user's review, in
`<run folder>/reviews.md`, and bring the whole list to the user in
one message. A merge or pull request edit request is never a routine
step. A worker sends one only after you tell it the user reviewed
that pull request, so tell it once the user has, and take the pull
request off the list. The worker then merges and reports once it has
merged.

Hand any other merge to a new Herdr session started with
`herdr-handoff`, never to a subagent of the hub. A subagent ends when
the hub's turn is interrupted, and the user cannot see it.

## Hand off the hub

When you hand the hub to a new session, write its handoff with
`herdr-handoff`, holding only the run's state, then start it with:

    hub handoff --run "<run folder>" <name> <handoff file>

It starts `<name>` in a tab of your workspace on the model of the last
`--model '...'` line in `rules.md`, so keep the hub start command there
in that form. It checks its inputs before it starts anything. Before it
sends the handoff, it writes your session review's handoff and adds to
the new hub's handoff how to close your tab and start that review. It
then reads back the reasoning effort of the new hub's first reply. Tell
the user the effort it printed, never one you assume. When it exits 2
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
- Write to the user only when the user must act or decide, in one
  short message per turn, by `status-updates`. Keep the detail in
  `<run folder>/notes.md`. The user reads your pane while news keeps
  arriving and cannot keep up with a fast stream.
- When the user's word differs from your handoff or this skill,
  follow the user and record the new rule in `rules.md`. You still
  never answer an approval or question shown in another session's
  pane.
- When your context grows past about 150k tokens, or after it is
  summarized, use the `herdr-handoff` skill to offer the user a new
  session. Offer once.

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
