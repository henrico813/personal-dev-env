---
name: status-updates
description: Writes status updates, progress reports, recaps and
  handoff messages that a reader new to the work understands on a
  phone. Each one leads with what changed and what the user must do,
  reports only checked results, and has no AI filler. Use for any
  update to the user, including the update after each step of
  delegated work, a recap after time away, and a short answer about
  an earlier update.
---

# Status Updates

The user reads updates on a phone, out of order, often after time
away. Write each one so a reader who missed every earlier message
knows what happened and what to do next. Write each sentence by the
`plain-direct-english` skill; this skill covers what an update says
and in what order. In delegated work, send one after each step, even
when the user has nothing to do. A batch of agents that run at once
counts as one step: a one-line note when it starts, and one update
when it is done.

## Write for a new reader

- Open with the project's name and what it does, in a few words.
- Say what each thing does for the user before its label. Write "the
  approvals-file pull request (P4, #383)", not "P4". Explain every
  term, such as "batch 2", in every message.
- Name the repository with each pull request, card and plan, since
  the user works on several, even when an update covers only one.
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
  itself, such as `less <path>`. A command that opens the card or
  board that links it does not count. Run the command once to see
  that it works.

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
2. The checker prints nothing. It adds the limits in
   [references/phone-limits.md](references/phone-limits.md) to the
   `plain-direct-english` checks. Without `draft.md`, it reads stdin:

   ```bash
   python3 ~/.agents/skills/status-updates/scripts/check_update.py draft.md
   ```

3. A new reader knows every label in the update.
4. The user's actions come early, each with its link. Each file's
   command opens that file, not its card.
5. Every question and approval still waiting is in the update.
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
When the user corrects how you write an update, recap or handoff,
follow [skill-lessons](../skill-lessons/SKILL.md) with the skill name
`status-updates`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
