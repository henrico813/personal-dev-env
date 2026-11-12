---
name: skill-development
description: Writes, changes and reviews agent skills so that each
  rule has one owner, no skill repeats or contradicts another, and a
  person can read, follow and debug the result. Use when creating,
  changing or reviewing a skill, its references or its routing line,
  or when turning saved lessons into skill text.
---

# Skill Development

Agents follow skills, and people keep them working. A rule written in
two skills drifts until the copies disagree, and an agent then
follows whichever copy it read last. This skill says where each rule
goes, how to write it so a person can follow it, and how to check a
change against every other skill. A script finds what a program can
find, and two fresh reviewers check the rest.

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
- An example: shows a rule the text states, and adds none of its own.
- Detail that only some runs need: a file in `references/`. Link it
  from SKILL.md and say when to read it. Give a reference over 100
  lines a Contents list.
- A step a program can do or check: a script in `scripts/`, tested in
  `ai/tests/`.
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
- Use the words other skills use for the same thing, such as
  "worktree" and "Lessons section". A new word makes the reader wonder
  whether it means something new.
- Wrap each Markdown line you add or change at 72 columns. Leave lines
  you did not change as they are.
- Leave out task details, such as plan and card IDs, pull request
  numbers and dates. Name a repository only when a step must run in
  it. The skill's reader missed the task.
- Write the description in the third person. Say what the skill does
  and when to use it, in under 1024 characters.
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
3. For a new skill, name its folder in lowercase words joined by
   hyphens, and give its frontmatter the same `name`. Give a skill
   that runs a whole workflow, such as `create-plan`, the frontmatter
   `pde-workflow: "true"` under `metadata:`. Vibe workers do one task,
   so they leave those skills out.
   - Add its files to Git. The installer copies every file Git tracks
     in `ai/skills/` to a full install. A client install gets only the
     skills listed in `chezmoi/.chezmoiexternal.toml.tmpl`, so add the
     skill there only when a client laptop needs it.
   - Add its routing line to `ai/AGENTS.md`, a row to the
     installed-layout table in `README.md`, its name to the skill list
     in `ai/README.md`, and a row to `ai/evals/skill-routing.md`.
   - Add `references/evaluation.md` with at least three cases, each
     also run with no skill to compare.
4. Run the check from the checkout's top folder, with the commit your
   branch started from:

   ```bash
   python3 ~/.agents/skills/skill-development/scripts/skill_check.py \
     --base "$(git merge-base main HEAD)"
   ```

   For a branch stacked on another, use that branch in place of
   `main`. Fix every error. For each repeat, replace the copy with a
   pointer to its owner, or write down why it stays, for the pull
   request.
5. Run the checks in `ai/evals/skill-routing.md` when its opening says
   to.
6. Get the two reviews in "Review a skill change". If you cannot start
   another agent, or another agent gave you this task and did not ask
   you to start reviewers, check the change yourself against Final
   checks instead. Then say in your report that the fresh reviews did
   not run, so whoever gave you the task arranges them.

## Turn saved lessons into skill text

Do this only when the user asks, as `skill-lessons` says.

1. In a worktree of the skills repository, list the lessons:

   ```bash
   skill-lessons promote-start <skill> --checkout <worktree>
   ```

2. Place each lesson's rule as "Where a rule goes" says. A lesson on a
   topic another skill owns goes to that owner. Write each rule in
   your own words.
3. Before you commit, check the skill the lessons came from. When it
   grew, add `--growth-reason "<why>"`:

   ```bash
   skill-lessons promote-check <skill> --checkout <worktree>
   ```

   It fails when lines added to that skill copy five words in a row
   from one of its lessons. It reads no other file, so check by hand
   that a rule you put in another file copies no five words in a row
   from its lesson.
4. Run the check script, the routing checks and the reviews, as steps
   4 to 6 of "Change a skill" say. When both reviews ran, open the
   pull request with the `pull-request` skill. Otherwise report as
   step 6 says, and open no pull request.
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
the change, and neither one starts another agent.

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
   Even after it loads this skill, it reads no other file and runs
   nothing.

Fix each required finding. When the fixes are large, run a new
reviewer of the same kind once more.

The consistency reviewer, and anyone who reviews a skill change under
the `review-plan` skill, checks the change against "Where a rule
goes", "Write for a person", Final checks and the guide checks
below. It uses the check script output that came with the change,
and reports a missing one as a finding.

Also check these rules from the guides in References:

- The SKILL.md body stays under 500 lines. Detail past that goes to
  `references/`.
- No paragraph explains what the agent already knows, such as what a
  PDF is.
- Each example is concrete, not abstract.
- Each script handles its errors and says what went wrong.

When a guide and this skill differ, follow this skill.

## Output format

- For a change: when you open a pull request, write it by the
  `pull-request` skill. Put the check script's last line in Testing,
  and the reason each kept repeat stays in its Changes line.
- For a review: one finding per item, required ones first, or the
  `review-plan` layout under that skill. Each finding gives the file
  and line and the rule it breaks. A conflict quotes both sides.

## Guardrails

- Edit skills in the checkout. Never edit the installed copies in
  `~/.agents/skills/`, since the installer replaces them.
- A change to `ai/AGENTS.md` changes what every session loads. Say so
  in the pull request's Overview, so the user reads it before merging.
- Report a conflict between skills the change does not touch to the
  user. Fix it in its own pull request, not this one.

## When a tool is missing

- If the check script cannot run, or exits 2 because the checkout has
  no `ai/skills` or `skills` folder, say so. Then check links, skill
  names, reference links and line widths by hand.
- If you cannot start another agent, do as step 6 of "Change a skill"
  says.
- If `skill-lessons` is not installed, turn no lessons into skill
  text, and say so.

## Final checks

- The check script prints `0 errors`, and each repeat is gone or has
  a reason in the pull request.
- Each rule you added has one owner, and every other mention points
  to it.
- Each reference file is linked from SKILL.md with when to read it.
- Each prose line you added is at most 72 columns.
- The change meets the guide checks in "Review a skill change".
- Both reviews ran, or your report says they did not.
- The skill names no plan, card or pull request, and no repository a
  step does not run in.

## References

- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

The guide checks in "Review a skill change" come from these guides.
Read one when a check is unclear:

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
lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
