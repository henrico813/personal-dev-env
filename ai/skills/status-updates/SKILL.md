---
name: status-updates
description: Writes status updates, progress reports, recaps and
  handoff messages that a reader new to the work understands on a
  phone. Each one leads with what changed and what the user must do,
  reports only checked results, and has no AI filler. Use for any
  update to the user, including the update after each step of
  delegated work, a recap after time away, and a short answer about
  an earlier update. Also use for every question to the user, which
  it asks in a short form made for a phone.
metadata:
  scope: personal
---

# Status Updates

The user reads updates on a phone, out of order, often after time
away. Write each one so a reader who missed every earlier message
knows what happened and what to do next. Write each sentence by the
`plain-direct-english` skill; this skill covers what an update says
and in what order. In delegated work, send one after each step, even
when the user has nothing to do. A batch of agents that run at once
counts as one step, as Restraint says.

Every question to the user, in an update or on its own, takes the
short form in
[references/waiting-on-you.md](references/waiting-on-you.md), except
the one plain sentence that Question numbers allows, and
each reply to one is matched to its question as that file says. A
hub shows the questions on its cards as `herdr-hub` says instead. A
question on its own also passes the checker in Review before sending.
It names the project only when the user may not know which work it
is about.

## Question numbers

Number questions `Q1` to `Q9`, so each reply key, such as `Q3A`, has
one digit and is quick to type on a phone. A hub's questions on its
cards skip this section: the kanban `card` command numbers them per
card, with reply keys such as `DEMO-516 Q3A`, and `herdr-hub` says
how to match a reply to one and ask which question it answers.

- A batch of questions starts with the first question you ask when
  no earlier question waits on the user. It ends when no question in
  it still waits: each one is answered, or dropped because no one
  needs its answer any more, such as when its card is closed.
- Each question keeps its number until its batch ends, even after
  its own answer. The user may still reply to an older message, and
  a key must then name one question. When the batch ends, its numbers
  are free to use again, and the next batch starts at `Q1`.
- A new question in the open batch takes the number after the
  highest one in it.
- When the open batch already has a `Q9`, hold each new question
  until the batch ends. A held question is not asked yet, so leave it
  out of each update except for one line that says how many are
  held, such as `Held: 2 questions, asked once Q1 to Q9 are
  answered.` Ask the held questions first in the next batch.
- A question about the batch, such as which question a reply
  answers, takes the next number too. When the batch already has a
  `Q9`, ask it instead in one plain sentence that names the choices,
  with no number. It skips the short form.
- A handoff or compaction does not end the open batch. Keep the
  numbers the handoff or your earlier messages give, and number new
  questions after the highest. When you cannot tell whether the last
  batch ended, number new questions after the highest number you
  see, and hold new questions past `Q9` as an item above says. When
  you hold a question this way, ask the user whether the earlier
  questions still wait, as a question about the batch: if none
  waits, the batch has ended and the held questions start at `Q1`.
- Match a reply key against the open batch first. Then match each
  reply as [references/waiting-on-you.md](references/waiting-on-you.md)
  says.

## Write for a new reader

- Open with the project's name and what it does, in a few words.
- Say what each thing does for the user before its label. Write "the
  approvals-file pull request (P4, #383)", not "P4". Explain every
  term, such as "batch 2", in every message.
- A reply key, such as `Q7A`, is a term too. When one comes up in a
  later message, such as one that confirms an answer, first say in
  words what the question was about or what the user picked: "fix
  branches are now named by skill, as you picked in `Q7A`", not
  "`Q7A` is done". The user cannot tell what a bare key read later
  stands for, and may take it for a new question.
- Name the repository the first time an update names each pull
  request, card or plan, such as "inkwell #18". Do this even when
  the update covers only one repository, since the user works on
  several.
- To explain how work behaves, open with a small concrete example.
  Give each rule's real effect in one sentence, and name the file or
  function that holds the rule. Say what the evidence leaves unproven.

## Order

1. What changed since the last update.
2. What the user must decide or do, numbered, in the order to do it.
   For a question, approval or anything else waiting on the user, read
   [references/waiting-on-you.md](references/waiting-on-you.md) first.
3. Detail, only as much as the user needs.

Each thing the user must open comes with its link, branch, worktree
path or command. Say where to run each command.

- For a plan or other file, give a command that opens that file
  itself, such as `nvim <path>`. A command that opens the card or
  board that links it does not count. Check that the file exists
  with `test -f <path>`. Do not run `nvim` itself, since it waits
  for keys.

## Real results

- Give only numbers and outcomes you checked, such as "31 of 32 tests
  pass", and say what you did not check. Leave out or mark unchecked
  any other claim. Never make up a number, link or time.
- Correct a wrong earlier claim in its own line.
- Leave out private details you read in notes.

## Restraint

- Leave out sections with nothing new. Put detail in a file or the
  pull request and link it. Bold only what the user must act on.
- Raise an optional side item, such as deleting an old branch, in
  one message only. In every later message, leave it out: no
  reminder, "no rush", "last call" or note that you will stop asking.
  Bring it back only when it blocks work.
- When several delegated agents run at once, say in one line that
  they started. Then send one update when the whole batch is done.
  Write sooner only when the user must decide something or work is
  blocked.
- Answer a short question, such as "what is batch 2?", in at most two
  sentences. Leave out the parts' labels; no list or walkthrough.

## Review before sending

Never send a first draft. Reread it as a reader who saw no earlier
message, fix each miss, and send it only when every check passes:

1. Each sentence passes the Final checks of `plain-direct-english`.
2. The checker prints nothing. One line may stay: `update fills N
   lines`, when the waiting questions alone fill more than 20 screen
   lines, as
   [references/waiting-on-you.md](references/waiting-on-you.md)
   says. The checker adds the question form in that file and the
   limits in
   [references/phone-limits.md](references/phone-limits.md) to the
   `plain-direct-english` checks. Without `draft.md`, it reads stdin:

   ```bash
   python3 ~/.agents/skills/status-updates/scripts/check_update.py draft.md
   ```

3. A new reader knows every label in the update.
4. The user's actions come early, each with its link. Each file's
   command opens that file, not its card.
5. Every question and approval still waiting is in the update, and
   each question is in the short form. Held questions show only as
   a count, as Question numbers says. A hub's questions on its cards
   show as `herdr-hub` says instead.
6. You checked every number, and the update names what you did not.
7. The update repeats no side item that an earlier message raised.

## References

- [references/examples.md](references/examples.md): read for an
  example of a progress update, a recap after time away, a list of
  pull requests to review, and a correction.
- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you write an update, recap, handoff or
question, follow [skill-fix](../skill-fix/SKILL.md) with the skill
name `status-updates`, and offer once to fix the skill's text.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
