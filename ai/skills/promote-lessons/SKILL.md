---
name: promote-lessons
description: Folds the lessons saved for one skill into that skill's
  source text, ships the change, and retires the lessons. Also turns
  one saved memory or preference into a rule in a skill or AGENTS.md.
  Use when the user asks to promote, fold in or clear out a skill's
  lessons, or to promote a memory into source-controlled guidance.
metadata:
  pde-workflow: "true"
---

# Promote Lessons

A lesson is a correction the user saved with `skill-lessons`. It lives
only on this machine, and a skill with many lessons gets long and easy
to misread. Promoting rewrites the lessons as rules in the skill's
source, ships that change, and then retires the lessons.

Promote one skill at a time, and only when the user asks. In the steps
below, SKILL is the skill's name, and CHECKOUT is the git checkout
whose `ai/skills/SKILL/` or `skills/SKILL/` folder holds its source.
Ask the user for CHECKOUT if you cannot find it.

## How the checkout ships changes

If the checkout's AGENTS.md or README says how to commit, push and
install its skills, follow it in steps 1 and 6. Otherwise:

- In step 1, update the default branch with `git pull --ff-only`,
  create a branch and worktree from it as your instructions say, and
  use the worktree as CHECKOUT.
- In step 6, open a pull request with the `pull-request` skill, and
  after it merges, install the skills as the checkout says.

## Steps

1. Start. Bring CHECKOUT up to date as the section above says, then
   run `skill-lessons promote-start SKILL --checkout CHECKOUT`. If it
   refuses, tell the user its reason in one line and stop.
2. Sort. Read the source SKILL.md in full, then put each lesson in one
   group:
   - Promote: it holds for every task the skill covers, or for a
     scope the skill has its own section for.
   - Keep: it is tied to one project, epic or ticket, or the user
     wants to see it work in more tasks first. Keep a lesson marked
     "likely stays a lesson" unless it clearly holds everywhere.

   Keep, and point out to the user, a lesson about a setup failure or
   a tool that seemed broken. Fixing the setup is the real cure.
3. Edit the source for each Promote lesson:
   - Write a general rule as an order, in your own words, with at most
     one clause of why. Leave out dates, names, PR numbers and quoted
     ticket text.
   - Put the rule in the section and step it changes. A lesson with a
     repeat count above zero goes in the step where the mistake
     happens.
   - Replace any sentence the rule contradicts, and merge duplicates.
     If the skill already says it, strengthen or move that line, or
     make no edit. The lesson is still retired.
   - Prefer tightening or replacing a line to adding one.

   Leave the Lessons section empty under its marker. For example, the
   lesson "(repo acme/widgets) Run make lint before pushing; CI
   rejected PR 482 for an unused import." becomes the rule "- Before
   you push, run `make lint`; CI rejects lint failures."
4. Check. Run `skill-lessons promote-check SKILL --checkout CHECKOUT`,
   fix what it reports, and run it again until it passes. Add
   `--growth-reason "<why>"` only when the skill must grow.
5. Show. Send one message short enough for a phone: each lesson ID
   with its group and a short reason, then the promote-check output
   exactly as printed. Ask for one reply:
   - `ok`: go on to step 6.
   - `edit: <change>`: make the change, then repeat steps 4 and 5.
   - `cancel`: restore the source with
     `git -C CHECKOUT restore <source path>`, retire nothing, and stop.
6. Ship. Commit with the `git-messages` skill, with no lesson IDs in
   the message. Then push and install as the section above says.
7. Retire. Run this with the Promote IDs, from CHECKOUT or from the
   default branch after you pull it:

       skill-lessons retire SKILL ID... --after-push --checkout CHECKOUT

   It refuses until the remote's default branch has the source as
   CHECKOUT's HEAD has it, so run it after the push or the merge. Tell
   the user the result in one line.

## From a memory

A memory is a preference saved in an agent's memory, not under a
skill. Promote one only when the user asks; saving it never asks.

1. Write it as one short rule an agent would follow in any task.
2. Work in a personal-dev-env worktree, from any repository, unless
   the user asks for a rule in the current one.
3. Search `ai/skills/` and pick the narrowest home:
   - The skill it fits, in the section and step it changes.
   - A new `ai/skills/<name>/SKILL.md` for a reusable behavior. List
     it where other skills are listed, such as the README tables,
     `ai/README.md`, `chezmoi/.chezmoiremove` and the installer test.
   - One short routing line in `ai/AGENTS.md`, which every session
     loads, so keep the steps in the skill.
4. Check that `git config user.email` matches the last commit's
   author, as `promote-start` does for lessons.
5. Show the diff, and on `ok` ship it as step 6 says. There is
   nothing to retire.

## Guardrails

- Retire only Promote lessons, and only with `--after-push`. Until the
  rule is on the default branch, the lesson is its only copy.
- Keep a lesson's private details, such as project names and ticket
  text, out of the skill, the commit and the pull request.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you sort, edit or ship promoted lessons,
follow [skill-lessons](../skill-lessons/SKILL.md) with the skill name
`promote-lessons`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
