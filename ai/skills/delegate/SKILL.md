---
name: delegate
description: Use when the user asks to have an agent do a task and then
  review the result, iterate on it, and bring it back for their review,
  such as "have an agent do X, review its work, iterate until it's
  ready for me", or "plan it, build it on the local model and test it
  for real", or "same process as the last skill". Do not use it for
  work you are asked to do yourself.
metadata:
  pde-workflow: "true"
---

# Delegate

Have a subagent do the task. Review its work at every stop, send fixes
back to the same agent, and repeat until it passes your review. Then
bring it to the user.

## Steps

1. Pick the subagent's model. Use the model the user names, or the
   one `model-routing` picks for the job. Brief the subagent: give it
   the goal, the inputs, what to leave alone, which instructions it
   must load, and the report it must return. When the report counts
   things, give the count you expect, such as "expect 2 clones". For
   a local job, brief the subagent as `model-routing` says instead.
2. When it reports, check that it loaded the instructions you named,
   and check its claims against the real files, tests and systems
   before trusting them.
3. Review the work. Send required fixes back to the same agent, and
   review again. Repeat until it passes. A planner's fixes go to a new
   planner instead. The new planner gets the plan and the reviewers'
   findings, so no planner's context grows each round. When a local
   job fails its check, retry and step up as `model-routing` says for
   a failed check, instead of sending fixes back.
4. Give the user a short summary: what was done, the evidence, and any
   decisions they need to make, with your recommendation.

## Plan, then build

When the user asks for a plan and a build, such as a new skill or
tool, follow this order. The skill each step names says how to do
that step.

1. A planner on the strongest model, as `model-routing` picks, writes
   the plan with `create-plan`. It marks each decision the user made
   `user-decided`.
2. Fresh agents review the plan with `review-plan`. Each fix round
   goes to a new planner, as step 3 of "Steps" says.
3. The user reviews the plan. Build nothing until they approve it.
   By `status-updates`, send one message: a command that opens the
   plan, a summary of at most 15 lines, and the open questions, each
   with your recommendation.
4. An agent builds it with `implement-plan` on the local model.
5. Fresh agents review the change with `review-plan`. Code also gets
   `code-documentation` and `behavior-focused-testing` reviewers.
   Skill text gets the reviews in "Review a skill change" of
   `skill-development`.
6. Test it as Guardrail 4 of `pull-request` says.
7. Open the pull request with `pull-request`.

To build without the user's plan review, use `auto-run` instead.

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
- Offer the user a new session as the section "Context Size" of
  `ai/AGENTS.md` says. Offer once.

## References

- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

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
