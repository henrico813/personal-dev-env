---
name: skill-lessons
description: Explains how to follow and render the lessons saved in a
  skill's Lessons section with the skill-lessons tool, before
  corrections became fixes to the skill's text. Use when a skill's
  Lessons section is empty after an install, or when moving saved
  lessons into skill text.
metadata:
  scope: personal
---

# Skill Lessons

A skill's `## Lessons` section shows lessons, the corrections the
user saved on this PC before corrections became fixes to the skill's
text. A new correction goes to the `skill-fix` skill, which commits
it to the skill's text, never here.

## Follow the lessons

1. Follow every lesson in the skill's Lessons section. Each is a
   correction the user made, so a lesson wins when it disagrees with
   the rest of that skill, except where the skill names an exception.
2. If the section is empty and the `skill-lessons` command is on
   `PATH`, run `skill-lessons render` once, since an install can
   reset the skill's file, and read the section again. The command
   takes no arguments and fills every installed skill. A section
   still empty after it has no lessons.

## When a tool or folder is missing

- If the `skill-lessons` command is not on `PATH`, follow the
  lessons shown and render nothing.
- If the skills folder is read-only, as in a Vibe worker, follow the
  lessons shown and render nothing.

## Guardrails

- Never edit a Lessons section by hand. `skill-lessons` fills it from
  the local store.
- A saved lesson moves into skill text as "Write a fix into skill
  text" in the `skill-development` skill says.
