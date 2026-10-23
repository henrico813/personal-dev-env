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
2. When it reports, check that it loaded the instructions you named,
   and check its claims against the real files, tests and systems
   before trusting them.
3. Review the work. Send required fixes back to the same agent, and
   review again. Repeat until it passes.
4. Give the user a short summary: what was done, the evidence, and any
   decisions they need to make, with your recommendation.

## Rules

- The subagent does the task itself. Do not tell it to use `delegate`.
- The agent that did the work never reviews it; you or a new agent do.
- Stop for the user before anything public or hard to undo, such as a
  push, merge, deploy or delete.
- If the agent hits something unexpected, have it stop and report
  rather than improvise a fix.
- When the work changes code comments, bring the comments to the user
  for review last, after behavior and names are done.
- Make minor decisions yourself and say so. Ask only about real ones.
- Keep updates short; put detail in files, not chat.

## Remembering corrections

Here "the user" is the human who asked for the work, not a parent
agent. Follow every lesson in the Lessons section below. Each is a
correction the user made, so a lesson wins when it disagrees with the
text above, except that you still stop for the user before anything
public or hard to undo. If the section is empty and `skill-lessons` is
installed, run `skill-lessons render` once, since an install can reset
this file, and read the section again.

When the user corrects how you brief, check or review a subagent's
work, still save it to memory as your instructions say. If
`skill-lessons` is installed, also offer once, in your next summary,
to save it here as a lesson. When the user says "remember", save it at
once and show the one line it prints:

    jq -n --rawfile text /dev/stdin \
      '{skill: "delegate", text: ($text | rtrimstr("\n"))}' \
      <<'EOF' | skill-lessons add -
    <rule>
    EOF

The rule goes on its own line, so apostrophes, quotes, backticks and
`$` are saved as written. If the user corrects a lesson again, add
`repeat_of: "L<n>"` inside the braces, with the ID from the end of that
lesson. If `skill-lessons` is not installed, say so once and save no
lesson. Lessons become rules in this text only through a pull request
the user asks for.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
