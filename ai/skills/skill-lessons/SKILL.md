---
name: skill-lessons
description: Explains how to follow, save and render the user's
  corrections kept as lessons in a skill's Lessons section with the
  skill-lessons tool. Use when a skill's Remembering corrections section
  links here, or when the user corrects work done under a skill that has
  a Lessons section.
---

# Skill Lessons

A skill with a `## Lessons` section keeps the user's corrections there
as lessons. That skill's Remembering corrections section gives the
inputs this page needs:

- the skill's name, for the command below
- which corrections count, such as "how you plan, write or review
  tests"
- any exception to "a lesson wins", such as "except that the Never
  list still holds"
- when to offer to save a lesson, such as "once, in your next summary"

Here "the user" is the human who asked for the work, not a parent
agent, unless the linking skill names someone else.

## Follow the lessons

1. Follow every lesson in the skill's Lessons section. Each is a
   correction the user made, so a lesson wins when it disagrees with
   the rest of that skill, except where the skill names an exception.
2. If the section is empty and `skill-lessons` is installed, run
   `skill-lessons render` once, since an install can reset the skill's
   file, and read the section again.

## Save a correction

1. When the user makes a correction the skill counts, still save it to
   memory as your instructions say.
2. If `skill-lessons` is installed, also offer to save it in that skill
   as a lesson, once and at the time the skill names.
3. Offer a lesson only for a rule an agent would apply correctly in
   another repository or task. Write it without facts true only of
   this task, such as its PR numbers, option labels, models on trial
   or one-time slips. If no such rule is left, offer no lesson.
4. When the user says "remember", save it at once and show the one line
   the command prints. Replace `<skill>` with the skill's name and
   `<rule>` with the rule:

       jq -n --rawfile text /dev/stdin \
         '{skill: "<skill>", text: ($text | rtrimstr("\n"))}' \
         <<'EOF' | skill-lessons add -
       <rule>
       EOF

   The rule goes on its own line, so apostrophes, quotes, backticks and
   `$` are saved as written.
5. If the user corrects a lesson again, add `repeat_of: "L<n>"` inside
   the braces, with the ID from the end of that lesson. For lesson L3:

       '{skill: "<skill>", repeat_of: "L3", text: ($text | rtrimstr("\n"))}'

## When a tool or folder is missing

- If `skill-lessons` is not installed, say so once and save no lesson.
- If the skills folder is read-only, as in a Vibe worker, follow the
  lessons shown and save or render nothing.

## Guardrails

- Never edit a Lessons section by hand. `skill-lessons` fills it from
  the local store.
- Lessons become rules in a skill's text only through a pull request
  the user asks for.
