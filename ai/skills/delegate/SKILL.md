---
name: delegate
description: Use when the user asks to have an agent do a task and then
  review the result, iterate on it, and bring it back for their review,
  such as "have an agent do X, review its work, iterate until it's
  ready for me", or "plan it, build it and test it for real", or "same process as the last skill". Do not use it for
  work you are asked to do yourself.
metadata:
  pde-workflow: "true"
  scope: personal
---

# Delegate

Have a subagent do the task. Review its work at every stop, send fixes
back to the same agent, and repeat until it passes your review. Then
bring it to the user.

## Steps

1. Pick the subagent's model with `model-routing`'s script, with the
   kind of the job you hand over, such as `plan-write`. Pass
   `--model` when the user names a model:

   ```sh
   python3 ~/.agents/skills/model-routing/scripts/pde_route.py <kind> --start <start>
   ```

   Begin the subagent's description with the printed run ID. Brief
   the subagent: give it the goal, the inputs, what to leave alone,
   which instructions it must load, and the report it must return.
   When the report counts things, give the count you expect, such as
   "expect 2 clones". For a local job, brief the subagent as
   `model-routing` says instead.
2. When it reports, check that it loaded the instructions you named,
   and check its claims against the real files, tests and systems
   before trusting them.
3. Review the work. Send required fixes back to the same agent, and
   review again. Repeat until it passes. A planner's fixes go to a new
   planner instead. The new planner gets the plan and the reviewers'
   findings, so no planner's context grows each round. When a local
   job fails its check, retry it with `--check-failed` as
   `model-routing` says, instead of sending fixes back.
4. Give the user a short summary: what was done, the evidence, and any
   decisions they need to make, with your recommendation.

## Plan, then build

When the user asks for a plan and a build, such as a new skill or
tool, follow this order. The skill each step names says how to do
that step.

1. A planner on the `plan-write` model, as `model-routing` picks,
   writes the plan with `create-plan`. It marks each decision the
   user made `user-decided`. When the user asks for a local draft,
   pick this planner's model as a `plan-draft` job instead. Each
   later planner stays `plan-write`, as `model-routing` says.
2. Fresh agents review the plan with `review-plan`. Each fix round
   goes to a new planner, as the review step of "Steps" says.
3. The user reviews the plan. Build nothing until they approve it.
   By `status-updates`, send one message: a command that opens the
   plan, a summary of at most 15 lines, and the open questions, each
   with your recommendation.
4. An agent builds it with `implement-plan` on the `step-code`
   model, as `model-routing` picks.
5. Start fresh agents to review the change. The agent that built it
   reviews none of it. One reviewer loads `review-plan`. Code also
   gets a `code-documentation` reviewer and a
   `behavior-focused-testing` reviewer. Skill text gets the reviews
   in "Review a skill change" of `skill-development`.
6. Test it as Guardrail 4 of `pull-request` says.
7. Open the pull request with `pull-request`. Before you open it,
   check that each review in step 5 ran. Name each one in your
   report to the user.

To build without the user's plan review, use `auto-run` instead.

## Rules

- The subagent does the task itself. Do not tell it to use `delegate`.
- The agent that did the work never reviews it; you or a new agent do.
- Stop for the user before anything public or hard to undo, such as a
  push, merge, deploy to a live system or delete.
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
subagent's work, follow [skill-fix](../skill-fix/SKILL.md) with the
skill name `delegate`, and offer once, in your next summary, to fix the
skill's text.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
