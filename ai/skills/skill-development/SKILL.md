---
name: skill-development
description: Writes, changes and reviews agent skills so that each
  rule has one owner, no skill repeats or contradicts another, and a
  person can read, follow and debug the result. Use when creating,
  changing or reviewing a skill, its references or its routing line,
  or when turning saved lessons into skill text. Do not use it for
  the wording of one sentence; plain-direct-english owns that.
metadata:
  scope: personal
---

# Skill Development

Agents follow skills, and people keep them working. A rule written in
two skills drifts until the copies disagree, and an agent then
follows whichever copy it read last. This skill says where each rule
goes, how to write it so a person can follow it, and how to check a
change against every other skill. A script finds mistakes such as
broken links, and two fresh reviewers check the rest.

## Inputs

- The skill to create, change or review, or the saved lessons to turn
  into skill text.
- A checkout of the repository that holds the skills, in `ai/skills/`
  or `skills/`.
- For a review, the diff and the commit it starts from.

## Where a rule goes

Each rule has one owner: one skill, `ai/AGENTS.md`, or one
repository's own `AGENTS.md`. Every other place that needs the rule
names the owner instead of copying it, such as "open it with the
`pull-request` skill". A copy goes stale when only the owner changes.

- When to load a skill: its description, and its routing line in
  `ai/AGENTS.md` when it has one, say when a session loads it. Another
  skill may say at which of its own steps to load a skill it needs,
  such as "Load `git-messages` before writing a title".
- A rule every session needs, whatever the task: `ai/AGENTS.md`. Every
  session loads it, so keep it to short rules and routing lines. Edit
  `ai/AGENTS.md`, never `~/.claude/CLAUDE.md`, which is made from it.
- A rule for one repository: that repository's own `AGENTS.md`, never
  a shared skill.
- A rule another skill owns: point to that skill. To change the rule,
  change it in its owner.
- A rule for some tasks that no skill covers: a new skill, with a
  routing line in `ai/AGENTS.md` that says which tasks load it.
- A rule every run of the skill needs: the main text of SKILL.md, next
  to the rules on the same topic.
- An example: a concrete case that shows a rule the text states. It
  adds no rule of its own.
- Detail that only some runs need: a file in `references/`. Link it
  from SKILL.md and say when to read it. Give a reference over 100
  lines a Contents list. Keep the SKILL.md body under 500 lines by
  moving such detail there. Both numbers come from Anthropic's "Skill
  authoring best practices".
- A step a program can do or check: a script in `scripts/`, tested in
  `ai/tests/`. Make the script handle its errors and say what went
  wrong.
- A saved lesson: the Lessons section, filled only by `skill-lessons`.
- A rule that is easy to miss: also a line in Final checks.

To find a rule's owner, search every skill and `ai/AGENTS.md` for the
rule's key words, then read each skill that matches:

```bash
rg -n -i "<key words>" ai/skills ai/AGENTS.md
```

The owner is the skill whose description covers the topic. When two
skills state the same rule, keep it in the owner and replace the
other copy with a pointer.

## Write for a person

- Write each sentence by the `plain-direct-english` skill.
- Put one rule in each list item.
- Leave out what the agent already knows, such as what a PDF is.
- Use the words other skills use for the same thing, such as
  "worktree" and "Lessons section". A new word makes the reader wonder
  whether it means something new.
- Wrap each prose line you add or change at 72 columns. Code blocks,
  table rows, headings and a line that holds only one long link may be
  longer. Leave lines you did not change as they are. At 72 columns a
  phone shows each line whole.
- Leave out task details, such as plan and card IDs, pull request
  numbers and dates. Name a repository only when a step must run in
  it. The skill's reader was not part of the task, so these details
  tell them nothing.
- Write the description in the third person. Say what the skill does
  and when to use it, in under 1024 characters, the most the Agent
  Skills format allows.
- Say when not to use the skill, in its description or in a line
  near the top. Name the nearby task and the skill that owns it, as
  this skill's description does for the wording of one sentence.
  Without that line, an agent may load the skill for a task another
  skill owns. Add that line to each new skill, and to each skill
  whose description you change.
- Give a rule whose reason is not obvious a short reason beside it,
  such as "since the installer replaces them" in Guardrails. A reader
  who cannot see why a rule exists cannot tell whether it is safe to
  drop.
- Keep made-up skill names and example links inside code blocks. The
  check script reads names and links in the text as real ones.
- Give a new skill the sections it needs from this list, in this
  order: Inputs, the steps, Output format, Guardrails, When a tool is
  missing, Final checks, References, Remembering corrections and an
  empty Lessons section.

## Change a skill

1. Read the skill in full. Then read each skill that names it and each
   skill on the same topic:

   ```bash
   rg -l "<skill name>" ai/skills ai/AGENTS.md
   ```

   Read the Lessons section of each one's installed copy in
   `~/.agents/skills/` too, since a lesson wins over the text.
2. Place each rule you add or change as "Where a rule goes" says.
3. For a new skill, also do what "Add a new skill" says.
4. Check and review the change, and open the pull request, as "Check
   and review a change" says.

## Add a new skill

A new skill needs these on top of the steps in "Change a skill":

- Name its folder in lowercase words joined by hyphens, and give its
  frontmatter the same `name`.
- Give a skill that runs a whole workflow, such as `create-plan`, the
  frontmatter `pde-workflow: "true"` under `metadata:`. Vibe runs
  agents that each do one task, so it leaves those skills out.
- Give every skill `scope: personal` under `metadata:`. In a
  repository that keeps its skills in a top-level `skills` folder, it
  gets `scope: work` instead. The check script refuses a missing or
  wrong tag.
- Add its files to Git, and its routing line to `ai/AGENTS.md`.
- List it everywhere the repository's own `AGENTS.md` says a new
  skill goes, such as install lists and READMEs.
- Add `references/evaluation.md` with at least three cases, each also
  run with no skill to compare. Anthropic's "Skill authoring best
  practices" asks for three.

## Check and review a change

1. Run the check from the checkout's top folder, with the commit your
   branch started from:

   ```bash
   python3 ~/.agents/skills/skill-development/scripts/skill_check.py \
     --base "$(git merge-base main HEAD)"
   ```

   For a branch stacked on another branch, use the parent branch in
   place of `main`. Fix every error. For each repeat, replace the copy
   with a pointer to its owner, or write down why it stays. The pull
   request gives that reason in Changes.
2. Run the checks in `ai/evals/skill-routing.md` when its opening says
   to.
3. Get the two reviews in "Review a skill change".
4. Open the pull request with the `pull-request` skill only after both
   reviews ran.

## Turn saved lessons into skill text

Do this only when the user asks, as `skill-lessons` says.

1. In a worktree of the skills repository, list the lessons:

   ```bash
   skill-lessons promote-start <skill> --checkout <worktree>
   ```

2. Place each lesson's rule as "Where a rule goes" says. A lesson on a
   topic another skill owns goes to that owner. Write each rule in
   your own words.
3. Before you commit, check the skill the lessons came from. When the
   skill now has more lines than before, add
   `--growth-reason "<why>"`:

   ```bash
   skill-lessons promote-check <skill> --checkout <worktree>
   ```

   It fails when lines added to that skill copy five words in a row
   from one of its lessons. It reads no other file, so check by hand
   that a rule you put in another file copies no five words in a row
   from its lesson.
4. Check and review the change, and open the pull request, as "Check
   and review a change" says.
5. After the pull request merges, retire the lessons:

   ```bash
   skill-lessons retire <skill> <id>... --after-push \
     --checkout <worktree>
   ```

   `--after-push` refuses until the default branch has your branch's
   copy of the lessons' own skill file. When that file did not change,
   it passes before the merge too, so check first that the pull
   request merged.

## Review a skill change

The author of the change starts two fresh agents. Neither one wrote
the change, and neither one starts another agent. The two find
different problems: one compares the change with the other skills,
and the other reads it as someone who never saw the task.

When another agent gave you this task and did not ask you to start
reviewers, start none. Check the change yourself against Final checks
instead. Then say in your report that the fresh reviews did not run,
so whoever gave you the task arranges them.

1. A consistency reviewer gets the diff, the check script's output and
   `ai/AGENTS.md`. It reads every skill on the same topics as the
   change, and the Lessons sections of their installed copies. It
   reports each conflict, each repeat and each thing that two skills
   name with different words. Each finding quotes both sides with
   their file and line, and names the owner. No script finds these
   conflicts, because two skills can disagree in different words. One
   skill may say to ask an open question once while another says to
   repeat it in every update.
2. A new-reader reviewer gets only the changed skill's files, never
   the task. It reports each sentence it cannot follow, each term it
   does not know, and each step where it would not know what to do.
   It also reports each rule whose reason is neither obvious nor
   given beside it.
   If it loads this skill, it still reads no file outside the changed
   skill and runs no command.

Fix each required finding. When the fixes are large, run a new
reviewer of the same kind once more.

The consistency reviewer, and anyone who reviews a skill change under
the `review-plan` skill, checks the change against "Where a rule
goes", "Write for a person" and Final checks. These reviewers use the
check script output that came with the change. One who did not get
that output reports the missing output as a finding.

## Output format

- For a change: when you open a pull request, write it by the
  `pull-request` skill. Put the check script's last line in Testing.
  In Changes, give the reason each kept repeat stays.
- For a review: one finding per item, required ones first, or the
  `review-plan` layout under that skill. Each finding gives the file
  and line and the rule it breaks. A conflict quotes both sides.

## Guardrails

- Edit skills in the checkout. Never edit the installed copies in
  `~/.agents/skills/`, since the installer replaces them.
- A change to `ai/AGENTS.md` changes what every session loads. Say so
  in the pull request's Overview, so the user reads it before merging.
- Report to the user a conflict between skills the change does not
  touch. Fix it in its own pull request, not this one.

## When a tool is missing

- If the check script cannot run, or exits 2 because the checkout has
  no `ai/skills` or `skills` folder, say so. Then check by hand each
  error that the `check_skills` docstring in `scripts/skill_check.py`
  lists. The consistency reviewer reports repeats.
- If you cannot start another agent, check the change yourself
  against Final checks instead of the two reviews. Then say in your
  report that the fresh reviews did not run, so whoever gave you the
  task arranges them.
- If `skill-lessons` is not installed, turn no lessons into skill
  text, and say so.

## Final checks

- The check script prints `0 errors`, and each repeat is gone or has
  a reason in the pull request.
- Each rule you added has one owner, and every other mention points
  to it.
- Each reference file is linked from SKILL.md with when to read it.
- A new skill is listed everywhere the repository's `AGENTS.md` says
  a new skill goes.
- Both reviews ran, or your report says they did not.
- The skill names no plan, card or pull request, and no repository a
  step does not run in.

## References

- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

Some rules above come from these guides. Read one when a rule above
is unclear. When a guide and this skill differ, follow this skill.

- Anthropic's "The Complete Guide to Building Skills for Claude", at
  <https://resources.anthropic.com/hubfs/The-Complete-Guide-to-Building-Skill-for-Claude.pdf>.
  Its checklist is "Reference A: Quick checklist".
- Anthropic's "Skill authoring best practices", at
  <https://platform.claude.com/docs/en/agents-and-tools/agent-skills/best-practices>.
  Its checklist is "Checklist for effective Skills".
- OpenAI Academy's "Using skills", at
  <https://openai.com/academy/skills/>. Its section "What's a
  SKILL.md file?" lists the parts of a skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you write, place or review skill text,
follow [skill-lessons](../skill-lessons/SKILL.md) with the skill name
`skill-development`, and offer once to save the correction as a
lesson. A correction to the wording of one sentence belongs to
`plain-direct-english` instead.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
