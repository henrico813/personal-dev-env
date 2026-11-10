---
name: delegate
description: Use when the user asks to have an agent do a task and then
  review the result, iterate on it, and bring it back for their review,
  such as "have an agent do X, review its work, iterate until it's
  ready for me". Do not use it for work you are asked to do yourself.
metadata:
  pde-workflow: "true"
---

# Delegate

Have a subagent do the task. Review its work at every stop, send fixes
back to the same agent, or to a fresh planner for a plan, and repeat
until it passes your review. Then bring it to the user.

## Steps

1. Brief a subagent. Use the model the user names, or the one their
   notes give for that kind of work. Give it the goal, the inputs,
   what to leave alone, which instructions it must load, and the report
   it must return. When the report counts things, give the count you
   expect, such as "expect 2 clones".
2. When it reports, check that it loaded the instructions you named,
   and check its claims against the real files, tests and systems
   before trusting them.
3. Review the work. Send required fixes back to the same agent, and
   review again. Repeat until it passes. Plan fixes are the
   exception: give a new planner the plan's path and what the
   reviewers found, and never resume the old one, whose context grows
   every round.

   For implementation, any review another skill requires from a fresh
   subagent, for example the test review in `behavior-focused-testing`
   and the code-only check in `code-documentation`, gets a new agent
   that sees only the inputs that skill lists. An agent that read the
   plan cannot do it. Comparing the commit with the plan diff that
   was reviewed is no code review either, so review the commit as
   deeply as `review-plan` asks.
4. Give the user a short summary: what was done, the evidence, and any
   decisions they need to make, with your recommendation.

## Rules

- The subagent does the task itself. Do not tell it to use `delegate`.
- The agent that did the work never reviews it; you or a new agent do.
- Stop for the user before anything public or hard to undo, such as a
  push, merge, deploy or delete.
- If the agent hits something unexpected, have it stop and report
  rather than improvise a fix.
- Check a count a second way, such as `ls` and `git worktree list`,
  before you report it, and tell the agent to do the same.
- When the work changes code comments, bring the comments to the user
  for review last, after behavior and names are done.
- Make minor decisions yourself and say so. Ask only about real ones.
- Write each update and the final summary by the `status-updates`
  skill.
- When your context grows past about 150k tokens, or after it is
  summarized, use the `herdr-handoff` skill to offer the user a new
  session. Offer once.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that you still stop for the user before anything public or hard
to undo. When the user corrects how you brief, check or review a
subagent's work, follow [skill-lessons](../skill-lessons/SKILL.md) with
the skill name `delegate`, and offer once, in your next summary, to
save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
