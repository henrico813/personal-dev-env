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
back to the same agent, and repeat until it passes your review. Then
bring it to the user.

## Steps

1. Brief a subagent. Use the model the user names, or the one their
   notes give for that kind of work. Give it the goal, the inputs,
   what to leave alone, which instructions it must load, and the report
   it must return.
2. When it reports, check its claims against the real files, tests and
   systems before trusting them.
3. Review the work. Send required fixes back to the same agent, and
   review again. Repeat until it passes.
4. Give the user a short summary: what was done, the evidence, and any
   decisions they need to make, with your recommendation.

## Rules

- Stop for the user before anything public or hard to undo, such as a
  push, merge, deploy or delete.
- If the agent hits something unexpected, have it stop and report
  rather than improvise a fix.
- Make minor decisions yourself and say so. Ask only about real ones.
- Keep updates short; put detail in files, not chat.
