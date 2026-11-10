# Adopt work already under way

Use this when an agent session in Herdr has been working for a while
and has no card yet. **Adopting** makes a card that starts straight in
`active` (a worker is on it) or `review` (the work waits only on
you), instead of walking through `draft` and `ready`. Only you
can adopt, so every command here passes `--user`.

## 1. Pick the stage
---

Read the session's pane or its latest report file.

- The worker can go on alone, such as while it builds: adopt into
  `active`.
- The work waits only on you, such as an open pull request, a plan to
  approve or a question: adopt into `review`.

## 2. Make the card
---

Name the project folder, a short title, and a reason for the Log:

```sh
card new DevEnv "Fix login timeout" --adopt active --user --by user --why "session fix-login running since 11-03" --session fix-login
```

`--session fix-login` records the Herdr agent `fix-login`'s session
in the card's `sessions` field, such as `opencode ses_...`, so the
card points at the session that does the work.

Add `--kind research` when the work ends in a document or a decision
rather than a change. `card` prints the new card's name, such as
`PDEV-KB-013 Fix login timeout`. The card's Log line reads
`- 2026-11-06 active: adopted by user for the user. session ...`,
then `- 2026-11-06 session: fix-login (opencode ses_...)`.

Adoption skips the work limit and the check for acceptance criteria,
since the work is already running. The board can then show more than 8
cards in `active` plus `review`. Until the count drops below 8, the
staff agent starts no new card.

## 3. Fill in Context and Acceptance Criteria
---

Open the card in Obsidian. Write why the work matters under
**Context**, and one `- [ ]` line per condition under **Acceptance
Criteria**, from what the session is doing. Without criteria there is
nothing to check the work against when it reaches `review`.

## 4. Link its plan and pull request
---

If the session works from a plan, or has opened a pull request, link
them:

```sh
card link PDEV-KB-013 "PDEV-415 Login timeout"
```

```sh
card link PDEV-KB-013 https://github.com/OWNER/REPO/pull/450
```

A plan note name goes into the `plans` field, and a URL into `prs`.

## 5. Name the session after the card
---

A worker session is named after its card's key and its role, in lower
case, so the board and Herdr's agent list point at each other:

```sh
herdr agent rename TARGET pdev-kb-013-impl
```

`TARGET` is the agent as `herdr agent list` shows it.

## 6. Pause the work to close its session
---

Skip this step to keep the session running. To close it before its
work is done, first have it write a handoff, as the `herdr-handoff`
skill says, listing every file it leaves and every question waiting
on you. Closing the session changes no file. Then move an `active`
card back to `ready` and name the handoff:

```sh
card move PDEV-KB-013 ready --user --by user --why "paused; handoff ~/.local/state/pde/handoffs/20261106-pdev-kb-013-impl.md"
```

A card in `review` cannot move to `ready`. Leave it there, and name
the handoff in a note:

```sh
card note PDEV-KB-013 --by user "paused; handoff ~/.local/state/pde/handoffs/20261106-pdev-kb-013-impl.md"
```

A paused `ready` card starts again like any ready card, from its
handoff. A paused `review` card starts again once you answer: it
moves back to `active`, and a new session starts from its handoff.

## Check
---

Run `card board`. The new card is counted under its stage. A card
adopted into `review` also shows in the review queue, dated today.
