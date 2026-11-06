# Adopt work already under way

Use this when an agent session in Herdr has been working for a while
and has no card yet. **Adopting** makes a card that starts straight in
`active` (a worker is on it) or `review` (the work waits for you to
accept it), instead of walking through `draft` and `ready`. Only you
can adopt, so every command here passes `--user`.

## 1. Pick the stage
---

Read the session's pane or its latest report file.

- The worker is still building, or waits on a question: adopt into
  `active`.
- The worker says it finished, and its pull request or branch exists
  with passing tests: adopt into `review`.

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

## Check
---

Run `card board`. The new card is counted under its stage. A card
adopted into `review` also shows in the review queue, dated today.
