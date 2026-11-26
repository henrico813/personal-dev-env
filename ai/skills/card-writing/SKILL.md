---
name: card-writing
description: Writes and reviews the kanban cards that the `card`
  command keeps as notes in the user's Obsidian vault. It searches
  the open cards first, keeps the user's words as given, marks its
  own wording, and proposes acceptance criteria only when the user
  gave none. It shows the whole proposed card before `card new`
  runs. Use when a session needs a card written, or a card's title,
  Context or criteria reviewed. Not for stages, releases or board
  limits, which the `herdr-hub` skill owns.
metadata:
  scope: personal
---

# Card Writing

A card is one piece of work on the user's kanban board. This skill
owns its title and sections, and how a proposed card reaches the
user. `card new` makes the card in the `draft` stage; the Cards
section of the `herdr-hub` skill owns that stage, releases and the
board's limits. The hub also makes cards itself, and those follow
`herdr-hub`: its question cards hold a question for the user, and
its adopted cards cover work already running.

## Inputs

- Words the user dictates for the title, Context and criteria. The
  user may instead ask for a proposed card and give no words.
- The project, which `card new` calls PROJECT: the project folder's
  name, such as `DevEnv`, the same name `card list --project` takes.
  Ask which project the card belongs to when the user names none.
- The kind: `build` ends in a change to code, config or the running
  setup; `research` ends in a document or a decision. Ask which
  kind the user wants when the work fits both.
- The `card` command. `card --help` lists the subcommands, and
  `card new --help` shows the file layout `--from FILE` takes.

## Search existing cards

Before you write a proposed card, search the work's key words and
list the open cards, those not in `done` or `dropped`. Search even
when told to create nothing: an unsearched card can repeat one.

```bash
card grep PATTERN --project PROJECT
card list --project PROJECT --tabs
```

- `card list --tabs` prints key, stage, kind, project and title per
  line, so column 2 is the stage. Pipe it through `awk -F'\t'
  '$2 != "done" && $2 != "dropped"'` for open cards only.
- `card grep` prints `KEY Section: line` and no stage, so read each
  matched key's stage in the unfiltered `card list --tabs` lines.
- When an open card may already cover the work, report it under
  "Not part of the card" and ask whether to use it instead.
  Dictated words never go there; "Keep the user's words" says why.

## Write the proposed card

- The title names the outcome and reads like a commit title. `card`
  owns the title rules: `card new` refuses a title that breaks them
  and prints why. When that happens after the user approved the
  card, show the refusal and a new title, and get approval again.
- Context states the problem in one or two sentences. Start each
  Context line with `- `, as `card new --help` shows, since the hub
  reads only such lines. Add the benefit only when the problem alone
  shows no cost. Never justify a card by citing another card, a plan
  or who chose it: the reader needs the reason itself.
- The user sets the acceptance criteria, as `herdr-hub` says. Propose
  two or three short, checkable ones only when the user gave none,
  and mark each one as your wording, like a title you wrote.
- Each criterion is one line the card's own work finishes alone: a
  `build` card's names a result in use, a `research` card's the
  document or decision it ends in. Leave mechanisms to plans.
- No criterion is a README: a README is detail about the work, while
  a how-to doc is a result the user uses, so a how-to may be one.
- A card about work shared between harnesses, the programs that run
  an agent, says nothing about harnesses in its title, Context or
  criteria, not even "any harness" or "every harness": a name ties
  the work to one, and any mention drags harnesses into the card.
- Use the names the project already uses: a term its reader has not
  met looks like new work, and they stop to ask what it means.
- A card that lists steps of its own numbers them 1 to N in the
  work's order, with no phases or letters: one numbering lets a
  reader match one step across the card.

## Keep the user's words

- Keep words the user dictated as given. While drafting you may fix
  speech-to-text errors, case, punctuation and fillers. These fixes
  stay the user's words, so they get no "(my wording)" mark. Before
  any yes, fixes are just draft text.
- Never shorten a dictated title. A dictated sentence stays in the
  field the user dictated it for, even when it reads off topic.
  Never move it to "Not part of the card"; ask about it in a
  question below the card instead.
- After the user's yes, a plain fix such as a capital first letter
  or no final period needs no new approval; any other change does.
- Wrong user text means a fact or name you can show is wrong, such
  as a file name that does not exist. Propose changing only the
  wrong words. Show the old text and the new text below the card.
  Change the card only after the user says yes to that change.
- A title, Context line or criterion you wrote is your wording.
  Write "(my wording)" after it in the review message, never in the
  file for `card new`, since `card` copies section text as written.

## Show the proposed card and get approval

- The proposed card shows the project, the kind, the title and every
  section the card file will have, Open Questions included, as the
  Output format shows.
- Each later message about the card shows the whole card again, so
  the user reads one current text. An answer to a question is not
  approval: put it in the card, show the whole updated card, and
  wait for a new yes. Only a yes to the text shown makes the card.
- Any change to approved text cancels that approval, except the
  plain fixes "Keep the user's words" allows.
- Ask the user each question you have about the proposed card, below
  the card, in the short form the `status-updates` skill owns. Put a
  question only the worker, the session that later does the work,
  can answer while doing it, in its Open Questions section.
- Keep the message short and readable on a phone: cut each line a
  card does not need, and keep lines short.

## Make the card

1. Wait for the user to say yes to the card text you showed.
2. Run `mktemp -d ~/.local/state/pde-card.XXXXXX` and write the
   approved text there as a file in the `--from FILE` layout, with
   `- [ ]` lines under Acceptance Criteria. That folder survives a
   reboot and is not shared like `/tmp`.
3. Run `card new PROJECT --from FILE --by <your agent name>`. `--by`
   names who acts, for the Log: your agent name, such as this session's
   Herdr agent name, else a short name for yourself. `--user` records in
   the Log that the user asked for the card. Pass it only when the user
   tells you to; a yes to the card text alone is not that.
4. `card new` prints the new card's file name, whose first word is
   its key. Read the card back with `card show KEY`, and check it
   against the approved text. When it differs, show the user the
   difference, and after they say yes fix it with the commands in
   "Review an existing card" (title with `card set`).

When `card new` refuses the project because its folder has no
`Project code:` line, follow the Cards section of the `herdr-hub`
skill: that skill owns proposing and writing the line.

## Review an existing card

- Read the card with `card show KEY`, then apply the rules in
  "Write the proposed card" and "Keep the user's words" to it.
- Report each finding with the full text you propose, so the user
  reads the result, not a description of it.
- Change the card only after the user says yes to that text. `card
  edit KEY SECTION N "<text>" --by <your agent name>` replaces item N,
  and with `--remove` it removes item N; `card add KEY SECTION "<text>"`
  adds one; `card set KEY title "<text>"` changes the title and takes
  no `--by`. SECTION is one of Context, `Acceptance Criteria`, `Open
  Questions` and Decisions, quoted when it has a space, and N is the
  item's number in `card show KEY --section SECTION`, counting from 1.

## Output format

The file for `card new PROJECT --from FILE`. The project is the
command argument, so the file holds the kind and the sections:

```text
---
title: Retry failed uploads
kind: build
---

## Context
- Uploads fail on a slow link, and a failed upload never runs again.

## Acceptance Criteria
- [ ] A failed upload retries on its own.
- [ ] The user sees each retry in the log.

## Open Questions
- Does the upload code already queue a failed item?
```

The review message shows that card, marks each line you wrote, and
asks its question below it:

```text
Proposed card, project DevEnv, kind build:
title: Retry failed uploads (my wording)

## Context
- Uploads fail on a slow link, and a failed upload never runs again. (my wording)

## Acceptance Criteria
- [ ] A failed upload retries on its own. (my wording)
- [ ] The user sees each retry in the log. (my wording)

## Open Questions
- Does the upload code already queue a failed item? (my wording)

Nothing is made yet; say yes and I run `card new` for this card.

Not part of the card:
- The open card DE-012 may cover the same work.

**Q1. Retry uploads (DE-012): reuse the open card or add this one?**
- A: reuse DE-012, drop this card **(rec)**
- B: keep both cards

Reply like `Q1A`.
```

## Guardrails

- Never copy a plan or research note into a card section: the reader
  at the board needs only the card's own words; ask about such text
  below the card.
- Text the user dictates wins over that rule: keep dictated words
  that came from a plan, and ask about them below the proposed card.

## When a tool is missing

- When `card` is not installed, skip the search, and draft and show
  the card as usual. After the user's yes, write the approved text
  file from "Make the card" step 2 and give the user its path, so
  the user or a later session can run `card new --from` on it once
  `card` is installed.

## Final checks

- You ran `card grep`, and `card list --project PROJECT --tabs`,
  before writing the card, unless `card` is not installed.
- Words the user dictated are kept as given, and each line you
  wrote is marked "(my wording)" in the message, not in the file.
- Each Context line starts with `- `.
- The criteria are the user's, or yours and marked as yours. None is
  a README, and each names the card's own result.
- The message shows the whole card, notes go under "Not part of the
  card", and its questions take the `status-updates` form.
- The user approved the exact text shown before `card new` ran.

## References

- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you write, review or make a card, follow
[skill-fix](../skill-fix/SKILL.md) with the skill name
`card-writing`, and offer once to fix the skill's text.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
