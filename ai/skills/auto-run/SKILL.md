---
name: auto-run
description: Runs an auto run, in which a separate hub works a list of
  cards the user approved until each stops at a pull request or a
  question, without the user. A card that links an approved roadmap
  has its worker build that roadmap's pull requests as stacks. Use
  when the user asks for an auto run, asks to build several pull
  requests of an approved roadmap without reviewing each step, such as
  "build the rest of the roadmap overnight", or a handoff makes this
  session an auto run's hub or worker. To start one session, use
  herdr-handoff; for one task an agent does and you review, use
  delegate.
metadata:
  pde-workflow: "true"
  scope: personal
---

# Auto Run

An auto run works a list of cards the user approved, without the
user, until each card stops at a pull request the user can review or
at a question for the user. A card that links an approved design and
its roadmap page is a roadmap item: its worker builds every pull
request the roadmap page lists, as stacks. Any other card is a stack
of one pull request.

Three kinds of session take part. The front hub is the user's usual
hub: it briefs the user, gets the approval and starts the run. The
auto hub is a new session that starts the workers, reads their
reports and never messages the user. Each worker builds one card's
pull requests by [references/pr-loop.md](references/pr-loop.md). Both
hubs follow `herdr-hub`; this skill says what differs in a run.

The user's rules for every run:

- The front hub picks the cards, and the user edits the list.
- The run starts only when the user replies `approve run N`, where N
  is the run's number. That reply also lets the workers push their
  branches and file requests to open pull requests. A request is a
  `pde-gh-write` command that opens the pull request only after the
  user approves it in Herdr or in Moshi, the phone app that shows it.
- Each card stops at a pull request the user can read and review, or
  at a question for the user. The user reviews each pull request
  before it merges.
- Each run has a tier, T1, T2 or T3, that the user approves with the
  run. The front hub proposes the highest tier that any card in the
  run looks to have, by pr-loop.md's Review tiers, and the user may
  pick another.
- In any run, only a plan that changes the front hub's `rules.md`
  waits for the user's plan review before build. That file is the
  user's rules file in the front hub's run folder, which no pull
  request shows. Approving the run does not approve that plan. The
  auto hub's `common.md`, the rules file for workers, is not in any
  repository, so no plan changes it and none waits for it.
- In a T1 run, the worker builds every other plan. Each card ends at
  an open pull request, or at a question that parks it; while the
  user is away, its request waits in `requests.sh`. In a T2 or T3
  run, a plan that changes what the user sees daily, or adds a name
  the user types or reads, also stops. That stop is a question to
  the hub. `decisions.md` keeps the question for the user. It is not
  the user's plan review.
- The user reviews every other plan's pull request before it merges.
- Cards in the run may go past herdr-hub's limit on cards in
  `active` and `review`.
- The run ends when every card on the list has stopped.
- Nothing merges without the user's explicit yes.

## Before the run (front hub)

1. Make the run's folder with
   `mktemp -d ~/.local/state/herdr-hub.XXXXXX`, and note it in your
   `notes.md`. N is one more than the last auto run your notes or an
   `approved.md` in `~/.local/state/herdr-hub.*/` name; ask the user
   when unsure.
2. Pick the cards and roadmaps. Write the ready cards to `ready.md`
   and print them:

       hub auto ready --run "<run folder>"

   A roadmap the user wants built runs as a ready card that links its
   design and its roadmap page. Read both. When either is not
   approved, the card goes under `## Left out` with that reason.
3. Sort every ready card into one of three groups yourself, even
   when there is only one card: work-pc (the work runs on the work PC
   or uses its data), hands (it needs the user at a device, a login,
   a purchase or a choice before work can start) or alone (the rest).
   Read each card before you sort it.
4. Write `<run folder>/run.md`. Put under `## Run` the `alone` cards
   you pick, and every other ready card under `## Needs your hands`,
   `## Work PC` or `## Left out`, with a reason. Each line is the key,
   the project and the problem it solves, in plain words. Under the
   title, before the first `## ` heading, write the run tier you
   propose as a `Run tier:` line. Propose T1 when every card under
   `## Run` looks T1 by pr-loop.md's Review tiers. Otherwise propose
   the highest tier a card looks to have.

       # Auto run 2

       Run tier: T1

       ## Run
       - DEMO-KB-007 demo: failed uploads retry by themselves

       ## Needs your hands
       - DEMO-KB-009 demo: the new disk needs plugging in

5. Check the list until it exits 0:

       hub auto check --run "<run folder>"

   It exits 3, with one problem per line, when the list fails its
   check.
   A card the user picks may be ready, active or review. A card the
   user wants that is still a draft stays out until the
   user moves it to ready, or tells you to.
6. Find out when the user is away, and each of the user's rules that
   ends at a set time, such as a model limit until a quota resets.
   Ask the user when you do not know. Then send the briefing, by
   `status-updates`, as that turn's only message:

       Auto run 2: 3 cards run without you while you are away,
       until 07:00 on 12 Nov.
       1. demo, DEMO-KB-007: failed uploads retry by themselves.
       2. demo, DEMO-KB-011: the rest of the sync roadmap, 4 pull
          requests in one stack.
       ...
       Left out: DEMO-KB-009 (demo), the new disk needs your hands.
       Rules that end: planners use Opus until 02:00.
       Each card stops at a pull request or a question for you.
       Run tier: T1. Only a plan that changes your rules.md
       waits for your plan review; every other card ends at a pull
       request for your review, or at a question for you.
       Pull requests: while you are away, workers write each
       request to requests.sh. When you are back, tell me, then
       run `bash <run folder>/requests.sh` and approve each
       request in Herdr or Moshi, where each names the pull
       request it opens. A request expires four hours after it is
       filed. Merges wait for your yes after you review.
       Watch: say "status update" at any time. The end brief comes
       when every card has stopped.
       Reply "approve run 2", or edit the list.

   For a T2 or T3 run, also say that a plan that changes what the
   user sees daily, or adds a name the user types or reads, stops as
   a question for the user in `decisions.md`. That is not a plan
   review. The user reviews each other pull request before it
   merges, as pr-loop.md's Plans that wait says.

7. When the user replies, pass the reply word for word:

       hub auto approve --run "<run folder>" N "<the user's reply>"

   It saves the list and the reply in `approved.md` only when the
   reply is `approve run N`. Any other reply, such as "go" or "yes",
   approves nothing and exits 3: make the edits it asks for, check
   the list again, and send the briefing again.

   `approved.md` keeps run.md's whole text, so it keeps the run tier.
   When the user asks for another tier, change the `Run tier:` line,
   check the list again, and send the briefing again.

   When the user approved the run in their own words before the
   briefing, such as before going away, pass those words instead:

       hub auto approve --run "<run folder>" --given N "<the words>"

   They pass only when they name by key every card under `## Run`
   and no other card. The check reads card keys only. The run tier
   the user approved is the `Run tier:` line in `approved.md`. Never
   write `approved.md` by hand; when the words fail, the run waits
   for `approve run N`.

## Start the run (front hub)

Once `approved.md` exists, start the auto hub, agent `auto-hub-<N>`,
by `herdr-handoff`, in its own workspace labeled `auto run <N>`, on
the hub model your `rules.md` names. Its handoff says only: load
`auto-run` and `herdr-hub`; you are the auto hub of run N; the run
folder; the front hub's name and run folder; when the user's away
window ends; each rule that ends at a set time, with its end time;
and any other rule the user gave for this run.

## During the run (front hub)

When the user says "status update", run this and print its output
word for word, then your own news:

    hub auto now --run "<auto run folder>"

When the user answers a queued question before the end brief, write
the answer under its entry in the auto run's `decisions.md` and send
it to the auto hub with `hub send`. When a pull request that
changes a skill merges during the run, send the auto hub its name
the same way, since running sessions keep the old text. When the
user says they are back, send the auto hub that the away window
ended.

The auto hub writes to your run folder only its end brief, as
`report-auto-hub-<N>.md`, a note that the user should run
`requests.sh` again, or a blocker that stops the whole run.

## After the end brief (front hub)

Send the auto hub, with `hub send`, each piece of news about the
run's pull requests: a merge, a declined request, an answered comment
review, the user's answer to a parked question, and the feedback
below. The auto hub acts on it as its step 10 says.

When the user gives feedback on one of the run's pull requests, write
it as a list in `<auto run folder>/feedback.md`. Have a fresh agent
check every other open pull request of the run against that list, and
send what it finds to the auto hub with `hub send`.

## The auto hub

1. Run `hub auto now --run "<run folder>"`. When it exits 3, the run
   is not approved: write that to the front hub with
   `hub report --run "<front hub's run folder>" auto-hub-<N> <file>`,
   start nothing, and end your turn. Write every report to the front
   hub this way.
2. Check with `command -v` that `hub`, `card`, `herdr`, `wt`,
   `planner`, `pde-gh-write`, `pde-check`, `glimmer`, `nobs`, `jq`,
   `rg` and `python3` are on `PATH`; never run `pde-gh-write pr` to
   test it, since any `pr` command files a request. Check each model
   the workers will use, as `model-routing` says. When a tool is
   missing or no strongest model answers for the planners and
   workers, write a blocker to the front hub and end your turn.
3. Copy [references/worker-limits.md](references/worker-limits.md)
   to `<run folder>/common.md`, and fill in each `<...>`: your name,
   the run folder, the run tier from `approved.md`, the away window,
   each rule that ends at a set time as one line ending in
   `until <YYYY-MM-DD HH:MM>`, the models that answered, and the
   user's style rules, such as comment length or call layout, from
   your notes and from rules files such as `AGENTS.md`. It is the
   run's one rules file. Every handoff points to it and copies none
   of its rules.
   Also make the two files every worker appends to: an empty
   `<run folder>/requests.log`, and `<run folder>/requests.sh` with
   the line `# Run when back: bash requests.sh`. Write each of these
   three files only when it does not exist yet, as in
   `[ -e <file> ] || ...`. After a restart, do step 4 only for each
   card not yet active, do step 5 again, since it writes nothing,
   then go to step 6's check.
4. Move each card under `## Run` to active with
   `card move <key> active --user --why "auto run <N>, approved
   <time in approved.md>"`. Skip a card that `card show <key>`
   reports as active. Run one `card` call per shell call. Each
   call writes to the vault on the network drive and can be slow.
   When a call times out, only that card's move is in doubt.
5. Work out the stacks. Compare the files each card will change, from
   its card and the code; for a roadmap item, from its roadmap page.
   Cards that share a file run one after another: start the later
   worker after the earlier one pushes, and have it stack its branch
   on the earlier branch with `--base`. When an unmerged branch
   changes a card's files, the card stacks on that branch; a roadmap
   item's branch is one example. Name each
   stack in the end brief, since the lower pull request must open
   first. When the earlier card stops without pushing, do not start
   the later card. It counts as stopped, and the end brief lists it
   as waiting on the earlier one. A roadmap item's worker works out
   the stacks within its roadmap itself.
6. Start one worker per card by `herdr-handoff`, as agent
   `<key in lower case>-auto`, in its own workspace named as
   `herdr-hub`'s Sidebar names say. Each worker directs other agents,
   so it is an orchestrator. Start it on the model and reasoning
   level that `model-routing` gives an orchestrator.
   `hub auto now` reads its status file by the agent name. Its
   handoff says only: load `auto-run`; you are the worker of the
   card; read common.md first and follow pr-loop.md; the card; the
   branch it stacks on; and for a roadmap item, its design and
   roadmap page. After a restart, check
   `hub status` first and start no second worker for a card.
7. Watch the run. Start `hub next` as `herdr-hub`'s Wake on news
   says, with `--timeout` set to the seconds until the nearest of
   these times:
   - an `until` time in common.md that has not ended, the away
     line's included
   - four hours after the newest line for a PR folder in
     `<run folder>/requests.log`, when its pull request is not open
   - 30 minutes after a running worker's status file last changed

   On each wake, check all three. When a rule's time has passed, tell
   each running worker with `hub send` that the rule ended, and add
   `(ended)` to its line in common.md. When a request has expired,
   read `request.result` in its PR folder, the worker's folder for
   that pull request. The request's line in `requests.log` names that
   folder. ship.sh
   writes that line only when it files a new request. When
   `request.result` starts with `exit 3`, the request was still
   waiting, so add its ship.sh line to `<run folder>/requests.sh`
   with pr-loop.md's Ship snippet, which adds no second copy. A line
   stays in `requests.sh` after its request is approved, and
   rerunning the file is safe, since ship.sh skips approved and
   declined requests. So when the user is back, and each time a
   request expires after that, write the front hub a note that the
   user should run `bash <run folder>/requests.sh` again. You alone
   queue an expired request again; a worker adds a waiting parent's
   line only to keep it before its child's, as Ship says. Add no line
   for any other result:
   ship.sh files nothing after a decline or an interruption, and the
   end brief lists the request. When a worker's status file is more
   than 30 minutes old, check its session with `hub status`; when it
   stopped without a report, ask it with `hub send` for its report.
8. Handle reports as `herdr-hub` says, except: never message the
   user. Each question that pr-loop.md's Plans that wait sends to
   the user, and each blocking question you cannot answer, goes into
   `<run folder>/decisions.md`, with a title that starts with the
   card's key; run `hub brief --run "<run folder>"` to check the
   entry's form. Answer every other question from the user's rules;
   when none applies, pick the simplest choice and have the worker
   record what it loses as a known limit in the plan's notes.
   - A plan that changes the front hub's `rules.md` goes into
     `decisions.md` in any run, with the plan's path. The user reads
     the plan and answers through the front hub; only a yes lets it
     build.
   - The worker parks the pull request of each such question: it
     stops that pull request and its children until the answer
     comes, and keeps building other stacks. When the front hub
     passes the user's answer, send it to the worker with `hub send`
     while it runs. Otherwise start a session by `herdr-handoff` that
     resumes from the PR folder's `status.txt`.
   - In a T1 run, a question that Plans that wait would stop in a T2
     or T3 run stops nothing. The worker picks its recommendation and
     records it, as Plans that wait says.
9. After each report, run `hub auto now --run "<run folder>"` so
   `brief-now.md` stays current. When the front hub says a skill
   changed, tell each running worker to load it again. When the front
   hub says the user is back, mark the away window `(ended)` and tell
   each running worker. When finding why something failed takes more
   than two commands, hand the search to a subagent and read only its
   report.
10. When every card has stopped, write the end brief by
    `status-updates` and send it to the front hub with
    `hub report auto-hub-<N>`, as step 1 says. It holds, from the
    workers' last reports:
    - per card: its project, the problem it solves, the result,
      whether a comment review waits, and the checks run and not run
    - a table of the pull requests: what each does, its tier and its
      changed lines, and a link to each summary a worker wrote
    - each choice a worker made in place of a question in a T1 run,
      for the user to decide at the pull request review
    - the stacks, such as "A, B, C" and "A, D", each pull request on
      the one before
    - the worst bugs the reviews caught
    - each request's state (open, waiting, queued, held, declined,
      interrupted or parked) and when it expires
    - the morning command, `bash <run folder>/requests.sh`, when that
      file holds a request; each request in it waits up to about 90
      seconds for the user's answer, and running the file again files
      each expired request again
    - what the user must do, in order: run the morning command and
      approve each request, merge the parents of held pull requests,
      answer blocking questions, then do the comment reviews
    - for each held pull request, what to do after its parent
      merges, and that you start a new session for it when the front
      hub says the parent merged
    - the output of `hub brief --run "<run folder>"`
    - what went wrong in the run, the workers' own slips, and work
      left for later

    Then end your turn, but stay the run's hub. When the front hub
    sends news after the end brief, start a session by
    `herdr-handoff` for each pull request it touches, as pr-loop.md's
    After a pull request opens says: a fix for feedback, a held pull
    request whose parent merged, a child's comment review once its
    parent's is answered, parking the children of a declined request,
    or an answer to a parked question, as step 8 says. A session for
    a held pull request follows its item's page.

## References

- [references/worker-limits.md](references/worker-limits.md): read
  as the auto hub at step 3, which copies it to `common.md`, the
  run's rules file.
- [references/pr-loop.md](references/pr-loop.md): read as a worker,
  for the loop each pull request goes through, its review tiers and
  the plans that wait for the user's plan review.
- [references/build.md](references/build.md): read as a worker, for
  the commands each step of the loop runs.
- [references/prompts.md](references/prompts.md): read as a worker,
  for the briefs of planners and reviewers.
- [references/pitfalls.md](references/pitfalls.md): read as a worker
  when something goes wrong.
- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that pr-loop.md's Never list still holds and you still never
answer an approval or question shown in another session's pane. Here
"the user" is the human who asked for the run, not a parent agent.
When the user corrects how you brief, start, plan, build, review,
ship or report an auto run, follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`auto-run`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
