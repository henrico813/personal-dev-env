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
2. If `skill-lessons` is installed, check the rule a correction teaches
   before you offer it as a lesson. Offer it only if all three hold:
   - True everywhere: it holds in every language, repository and task,
     with no "unless", and the skill does not already say it. For a
     language skill, it only has to hold in every project in that
     language.
   - Readable cold: someone who missed the task can follow it. Use
     plain words, no project names or task details such as PR numbers
     or file paths, and at most 60 words.
   - Checked by someone else: an agent that did not write it has
     checked it, given only the lesson and the skill's text, not the
     task. If you can't start another agent, check it yourself and say
     so.

   When you check several corrections at once, expect most to fail.
3. Offer each lesson that passes, once, at the time the skill names.
4. When the user says "remember", save it at once and show the one line
   the command prints. Replace `<skill>` with the skill's name and
   `<rule>` with the rule:

       jq -n --rawfile text /dev/stdin \
         '{skill: "<skill>", text: ($text | rtrimstr("\n"))}' \
         <<'EOF' | skill-lessons add -
       <rule>
       EOF

   The rule goes on its own line, so apostrophes, quotes, backticks and
   `$` are saved as written. The user's own rule skips the checks in
   step 2, and you reword it only if `add` refuses it.
5. If the user corrects a lesson again, add `repeat_of: "L<n>"` inside
   the braces, with the ID from the end of that lesson. For lesson L3:

       '{skill: "<skill>", repeat_of: "L3", text: ($text | rtrimstr("\n"))}'

   `add` replaces the saved text with the text you send, so send it
   unchanged unless the user changed the rule.

## When a tool or folder is missing

- If `skill-lessons` is not installed, say so once and save no lesson.
- If the skills folder is read-only, as in a Vibe worker, follow the
  lessons shown and save or render nothing.

## Guardrails

- Never edit a Lessons section by hand. `skill-lessons` fills it from
  the local store.
- Lessons become rules in a skill's text only through a pull request
  the user asks for.
