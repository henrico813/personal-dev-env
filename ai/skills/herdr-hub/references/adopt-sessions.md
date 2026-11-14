# Adopt running sessions

Read this when sessions run with no card, such as when a hub starts on
a machine full of earlier sessions, or the user asks to turn sessions
into cards and close them. Adopting makes a card that starts in
`active`, `review` or `waiting-on-others`, skipping `draft` and
`ready`. Only the user
adopts, so the user approves the list before any card exists.

Copy this checklist into `notes.md` and tick it off:

```text
- [ ] 1. Take stock of each session
- [ ] 2. Close finished sessions
- [ ] 3. Project codes proposed
- [ ] 4. Adoption list and codes approved
- [ ] 5. Cards made
- [ ] 6. Each session kept, handed off or paused
```

## 1. Take stock

For each session, read its pane and its working folder. Note in the
run folder what it does, whether that work is finished, what it holds
unsaved, and whether it waits on the user. Unsaved means uncommitted
files, or commits on no remote branch. Run `git fetch` before you
count a commit as unpushed, since a stale remote branch makes pushed
commits look unpushed. When sessions share one checkout, you cannot
tell which session changed which file; note them once, for the
checkout. Never commit or push for a session: list real unpushed
commits, such as on a main branch, among the decisions for the user.

## 2. Close finished sessions

Close a session yourself, without asking, when its work is finished,
it holds nothing unsaved and nothing waits in its pane. Close a
workspace once nothing open is left in it. Keep every other session
open until step 6.

## 3. Project codes

Every project in the list needs a code. Propose one for each project
with none, as Cards in SKILL.md says. Put the projects and codes in
one short table at the top of the adoption list, so the user approves
both in one reply.

## 4. Adoption list

Write the list to `adoption-list.md` in the run folder, and tell the
user where it is, its counts and the decisions it needs.

- One card per piece of work, not per session. Name the sessions each
  card covers, its project, title, kind and stage, and what the
  sessions show.
- Stage `review` when the work waits only on the user, such as an
  open pull request, a plan to approve or a question. Stage `active`
  when the session can go on alone. Stage `waiting-on-others` when
  the user already reviewed the work and it waits only on people
  outside the user's team; its `--why` names them, as Cards in
  SKILL.md says. Never propose `draft` for running
  work: a draft card is one the user has not released yet.
- For each session, propose its next step from step 6: keep, hand off
  or pause. Propose pause when the user wants it closed, and hand off
  instead of keep past about 150K tokens of context.
- List the sessions that get no card, and why, such as work already
  merged.
- End with each decision for the user, with your recommendation.

## 5. Make the cards

Once the user approves the list, make each card:

    card new <Project> "<title>" --adopt <stage> --user \
      --session <agent> --why "<what the session shows>"

Add `--kind research` for work that ends in a document or a decision.
For a card that covers several sessions, add the others with
`card link <KEY> --session <agent>`. Put what the sessions found and
what is left in the card with `card add <KEY> Context "<text>"`.

Adoption skips the board's limits, 8 cards in active and 6 in
review, as `card board` shows, so the board can go over them. Start
no ready card until both counts are back within their limits.

## 6. Keep, hand off or pause

- Keep: rename the agent and its tab to `<card key>-<role>`, as
  Sidebar names says, and send it the rules for a worker the user
  hands you.
- Hand off: have the session write a handoff by `herdr-handoff` for a
  fresh session named `<card key>-<role>`, naming any uncommitted
  files that pass to it. Start that session, then close the old one.
- Pause: when the user wants a session closed before its work is
  done, have it write a handoff by `herdr-handoff` that pauses its
  card, and close it once it reports. Then move its card back to
  `ready`, or note a card in `review`, naming the handoff, as
  herdr-handoff's Record the handoff on its card shows. A paused
  `ready` card starts again like any ready card, from its handoff. A
  paused `review` card starts again once the user answers: move it to
  `active` with `--user` and start a session from its handoff. The
  card's `sessions` list keeps the old session for the user to reopen.

Files in a shared checkout stay as they are. Each handoff from that
checkout lists them, and so does a note on one card that uses the
checkout. Never commit them for a session.
