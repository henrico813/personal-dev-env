---
name: auto-run
description: Runs an auto run, in which a separate hub works a list of
  cards the user approved until each stops at a pull request or a
  question, without the user. Use when the user asks for an auto run,
  or a handoff makes this session an auto run's hub. The front hub
  briefs the user and starts the run; the auto hub runs its workers.
  To start one session, use herdr-handoff; to build a roadmap's pull
  requests, use roadmap-run.
metadata:
  pde-workflow: "true"
---

# Auto Run

An auto run works a list of cards the user approved, without the
user, until each card stops at a pull request the user can review or
at a question for the user. Two hubs take part. The front hub is the
user's usual hub: it briefs the user, gets the approval and starts
the run. The auto hub is a new session that starts the workers, reads
their reports and never messages the user. Both follow `herdr-hub`;
this skill says what differs in a run.

The user's rules for every run:

- The front hub picks the cards, and the user edits the list.
- The run starts only when the user replies `approve run N`.
- Each card stops at a pull request the user can read and review, or
  at a question for the user.
- A plan that two fresh reviewers call ready may go to build without
  the user's read, unless it changes one of the user's rules or what
  the user sees daily. Approving a card for the run does not lift
  that exception.
- Run cards may go past the card limits during the run.
- The run ends when every card on the list has stopped.
- Nothing merges without the user's explicit yes.

## Before the run (front hub)

1. Make the run's folder with
   `mktemp -d ~/.local/state/herdr-hub.XXXXXX`, and note it in your
   `notes.md`. N is one more than the last auto run your notes or an
   `approved.md` in `~/.local/state/herdr-hub.*/` name; ask the user
   when unsure.
2. Write the ready cards to `ready.md` and print them:

       hub auto ready --run "<run folder>"

3. Sort them with the cheap model, as `model-routing` picks:

       opencode run --standalone -m <model> \
         "$(cat <run folder>/ready.md)

       Sort each card into one group, one line per card as
       '<key> <group>: <reason>'. Groups: work-pc (the work runs on
       the work PC or uses its data), hands (it needs the user at a
       device, a login, a purchase or a choice before work can
       start), alone (the rest)."

   When the model does not answer, pick the next one as
   `model-routing` says. When none is left, or a card gets no line,
   sort the cards yourself by the same rule. Read each line against
   the card before you use it.
4. Write `<run folder>/run.md`. Put under `## Run` the `alone` cards
   you pick, and every other ready card under `## Needs your hands`,
   `## Work PC` or `## Left out`, with a reason. Each line is the key,
   the project and the problem it solves, in plain words:

       # Auto run 2

       ## Run
       - DEMO-KB-007 demo: failed uploads retry by themselves

       ## Needs your hands
       - DEMO-KB-009 demo: the new disk needs plugging in

5. Check the list until it exits 0:

       hub auto check --run "<run folder>"

   It exits 3, with one problem per line, when the list fails its
   check.
   A card the user wants that is still a draft stays out until the
   user moves it to ready, or tells you to.
6. Send the briefing, by `status-updates`, as that turn's only
   message:

       Auto run 2: 3 cards run without you.
       1. demo, DEMO-KB-007: failed uploads retry by themselves.
       ...
       Left out: DEMO-KB-009 (demo), the new disk needs your hands.
       Each card stops at a pull request or a question for you.
       Plans that change your rules or what you see daily wait for
       you.
       Pull requests: workers hold them. When every card has
       stopped, all go out together, and you approve each request
       by its ID in Herdr or Moshi. Merges wait for your yes after
       you review.
       Watch: say "status update" at any time. The end brief comes
       when every card has stopped.
       Reply "approve run 2", or edit the list.

7. When the user replies, pass the reply word for word:

       hub auto approve --run "<run folder>" N "<the user's reply>"

   It saves the list and the reply in `approved.md` only when the
   reply is `approve run N`. Any other reply, such as "go" or "yes",
   approves nothing and exits 3: make the edits it asks for, check
   the list again, and send the briefing again.

   When the user approved the run in their own words before the
   briefing, such as before going away, pass those words instead:

       hub auto approve --run "<run folder>" --given N "<the words>"

   They pass only when they name by key every card under `## Run`
   and no other card. Never write `approved.md` by hand; when the
   words fail, the run waits for `approve run N`.

## Start the run (front hub)

Once `approved.md` exists, start the auto hub, agent `auto-hub-<N>`,
by `herdr-handoff`, in its own workspace labeled `auto run <N>`, on
the hub model your `rules.md` names. Its handoff says only: load
`auto-run` and `herdr-hub`; you are the auto hub of run N; the run
folder; the front hub's name and run folder; and any rule the user
gave for this run.

## During the run (front hub)

When the user says "status update", run this and print its output
word for word, then your own news:

    hub auto now --run "<auto run folder>"

When the user answers a queued question before the end brief, write
the answer under its entry in the auto run's `decisions.md` and send
it to the auto hub with `hub send`. When a pull request that
changes a skill merges during the run, send the auto hub its name
the same way, since running sessions keep the old text.

The auto hub writes to your run folder only its end brief, as
`report-auto-hub-<N>.md`, or a blocker that stops the whole run.

## After the end brief (front hub)

When the user gives feedback on one of the run's pull requests, write
it as a list in `<auto run folder>/feedback.md`. Have a fresh agent
check every other open pull request of the run against that list, and
send what it finds to the auto hub with `hub send`. The auto hub has
each card's worker fix it through a fresh planner and fresh reviews
before the user reviews that pull request.

## The auto hub

1. Run `hub auto now --run "<run folder>"`. When it exits 3, the run
   is not approved: write that to the front hub with
   `hub report --run "<front hub's run folder>" auto-hub-<N> <file>`,
   start nothing, and end your turn. Write every report to the front
   hub this way.
2. Check with `command -v` that `hub`, `card`, `herdr`, `wt`,
   `planner` and `pde-gh-write` are on `PATH`; never run
   `pde-gh-write pr` to test it, since any `pr` command files a
   request. Check each model the workers will use, as
   `model-routing` says. When a tool is missing or no strongest model
   answers for the planners, write a blocker to the front hub and end
   your turn.
3. Copy [references/worker-limits.md](references/worker-limits.md)
   to `<run folder>/common.md`, with your name, the run folder and
   the models that answered as `<models>`. Each worker's handoff points
   to it, and adds only the card and anything the card needs.
4. Move each card under `## Run` to active with
   `card move <key> active --user --why "auto run <N>, approved
   <time in approved.md>"`, one `card` call per shell call: each
   writes to the vault on the network drive and can be slow, so a
   call that times out is the only one in doubt.
5. Start one worker per card by `herdr-handoff`, as agent
   `<key in lower case>-auto`, in its own workspace named as
   `herdr-hub`'s Sidebar names say. `hub auto now` reads its status
   file by the agent name. After a restart, check `hub status` first
   and start no second worker for a card.

   Before you start them, compare the files each card will change, from
   its card and the code. Cards that share a file run one after another:
   start the later worker after the earlier one pushes, and have it
   stack its branch on the earlier branch with `--base`. A card whose
   files an unmerged branch changes, such as the branch of the skill
   under test, stacks on that branch. Name each stack in the end brief,
   since the lower pull request must open first. When the earlier card
   stops without pushing, do not start the later card. It counts as
   stopped, and the end brief lists it as waiting on the earlier one.
6. Handle reports as `herdr-hub` says, except: never message the
   user. Answer from the user's rules first; when none applies, pick
   the simplest choice and have the worker record what it loses as a
   known limit in the plan's notes. A question whose answer changes
   what the user sees goes into `<run folder>/decisions.md`, in the
   form `hub brief` checks, with a title that starts with the card's
   key. That card stops.
7. After each report, run `hub auto now --run "<run folder>"` so
   `brief-now.md` stays current. When the front hub says a skill
   changed, tell each running worker to load it again. When finding
   why something failed takes more than two commands, hand the
   search to a subagent and read only its report.
8. When every card has stopped, tell each worker with a finished
   branch to send its pull request create request in merge order.
   A branch at the bottom of its stack sends its comment review
   right after; a stacked branch's review waits, as the worker limits
   say. Requests go one at a time and at least a
   minute apart, since Moshi refuses pushes that come too fast. Add
   each pull request to `<run folder>/reviews.md` with its approval
   ID. Then
   write the end brief by `status-updates` and send it to the front
   hub with `hub report auto-hub-<N>`, as step 1 says: per card, its
   project, the problem it
   solves, the result, whether a comment review waits, and the checks
   run and not run; then the output
   of `hub brief --run "<run folder>"`; then what went wrong in the
   run. A request expires four hours after it is filed, so the brief
   says when; the worker's rerun of the same `pde-gh-write` command
   after that files a fresh one. End your turn.

Read [references/evaluation.md](references/evaluation.md) only when
testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that you still never answer an approval or question shown in
another session's pane. When the user corrects how you brief, start
or run an auto run, follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`auto-run`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
