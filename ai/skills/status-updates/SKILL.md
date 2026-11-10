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
and in what order.

## When to use

Use it for every status update, progress report, recap or handoff
message to the user. In delegated work, send one after each step,
even when the user has nothing to do.

## Write for a new reader

- Open with the project's name and what it does, in a few words.
- Say what each thing does for the user before its label. Write "the
  approvals-file pull request (P4, #383)", not "P4". Explain every
  term, such as "batch 2", in every message.
- Say which repository or project each plan, card or pull request
  belongs to, even when the whole update is about one project,
  because the user works on several. For each, describe the user's
  problem it solves, even if you explained it earlier that day.
- Describe an approval request by its effect, such as opening or
  merging a given pull request in a given repository, and say where
  the user approves it. Never use its ID, which the user cannot see.
- When you explain work, start with the concrete task and a small
  example. Give each rule's real-world consequence in one plain
  sentence, say where the rule lives in the source, and say what the
  evidence does and does not prove.
- Keep private details you read in notes out of updates.

## Order

1. What changed since the last update.
2. What the user must decide or do, numbered, in the order to do it.
3. Detail, only as much as the user needs.

Each thing the user must open comes with its link, branch, worktree
path or command. A file comes with a command that opens precisely
that file, tried before you send it; opening something nearby, like
the task's card, does not count. Give each deadline as the
time left, worked out from what you know, such as "expires in about
two hours". Add a clock time only when you know the current time.

A question says what it asks, the choices, what each choice leads
to, and your recommendation, as item 2 of the recap below does. Until
the user answers, every update carries the whole question again,
choices and recommendation included, not just its name.

## Pull requests to review

Order the list as the pull requests will merge, numbered. For each,
give its repository, a line on the user's problem it fixes, how its
checks went and anything blocking it. Put the first one to review in
bold.

## Real results

- Give numbers and outcomes you checked, such as "31 of 32 tests
  pass". Say what you did not check, such as "not run on Windows".
- A claim you did not check is not a result. Leave it out or mark
  it unchecked. Never make up a number, link or time the facts do not
  give, such as a pull request number for work not yet opened.
- Recheck the current state of anything you would list as waiting
  on the user, pending or open; never reuse an old status. When it
  has since been approved, finished or merged, report that.
- Correct a wrong earlier claim in its own line: "Correction:
  yesterday I said the tests cover Windows paths. They do not; only
  Linux paths ran."

## Cut AI filler

Follow the filler list in
[plain-direct-english](../plain-direct-english/SKILL.md#cut-ai-filler).

## Phone first

- Keep an update to about 20 short lines. Leave out sections with
  nothing new. Put detail in a file or the pull request and link it.
- Use tables of at most three columns, and no paragraph over three
  lines.
- Keep each command under about 35 characters, one per code block,
  and say where to run it. Wrap a long one in a short script first.
- When the user must change a login, token or setting, give them a
  script that makes the change and then prints ok or missing, so they
  can see it worked before saying done.
- Bold only what the user must act on.

## Restraint

- Raise an optional side item, such as deleting an old branch, in
  one message only. In every later message, leave it out: no
  reminder, "no rush", "last call" or note that you will stop asking.
  Bring it back only when it blocks work.
- Decide minor things, such as a name, and say what you chose in one
  line. Ask only when a wrong guess would cause rework.
- Answer a short question, such as "what is batch 2?", in at most two
  sentences. Leave out the parts' labels; no list or walkthrough.

## A recap

A recap after time away has three parts: done, in progress, and
waiting on you. For example:

```text
Fernwood, the app that waters the garden on a timer, since Monday:

Done
- Skip watering when rain is forecast (fernwood #7): merged Tuesday.

In progress
- A separate schedule for each garden bed (fernwood #8): 18 of 19 tests
  pass. The failing test covers a bed with no plants; I am fixing
  it. Not tried on the real valve controller yet.

Waiting on you
1. Approve fernwood #9, which texts you when a valve sticks, in the
   approval app. The request expires in about three hours.
2. Should a bed with no plants be skipped or watered at half time?
   Skipping saves water; half time keeps new seeds damp. I recommend
   skipping. fernwood #8 finishes within a day either way.
```

For a progress update rewritten from an agent's shorthand, read
[references/examples.md](references/examples.md). Read
[references/evaluation.md](references/evaluation.md) only when testing
or revising this skill.

## Review before sending

Never send a first draft. Reread the whole draft as a reader who saw
no earlier message, and fix it before you send it:

1. Check each sentence against the Final checks of
   `plain-direct-english`, and load that skill if it is not loaded.
   Rewrite each sentence that fails, such as one that packs several
   conditions onto one claim or leaves out who acts.
2. Check the whole update against this list, and fix each miss:
   - A reader who saw no earlier message understands every label.
   - The user's actions come early, numbered, each with its link,
     path or command.
   - Every number was checked, and unchecked things are named.
   - No side item that an earlier message raised.
   - Nothing from the filler list in `plain-direct-english` is left.
   - Each plan, card and pull request names its repository, and each
     open question is there in full.
   - It fits a phone screen without a wall of text.
3. Read the fixed draft once more. Send it only when every check
   passes.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you write an update, recap or handoff,
follow [skill-lessons](../skill-lessons/SKILL.md) with the skill name
`status-updates`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
