---
name: obsidian-zettel
description: Use when creating a Zettel note, quick reference note, or durable technical note in the default Obsidian vault.
---

# Obsidian Zettel

Create a concise, reusable note in the default Obsidian vault. Use the vault's
actual Zettel template as the source of truth for metadata, placement, and
structure.

## Workflow

1. Resolve the vault with `pde vault path default`.
2. Read `<vault>/AGENTS.md` and the applicable guidance under
   `<vault>/500 Zettelkasten/` before creating a note.
3. Read `<vault>/000 Meta/001 Templates/001.05 Zettel-Note.md`.
4. Infer a precise title that makes the subject and scope clear. For transfer
   notes, name both the source and destination. Ask only when the title or
   topic is genuinely ambiguous.
5. Infer one existing or useful topic. Use `[[<topic>]]` in frontmatter.
6. Render the template's final result directly. Do not require the user to
   operate Obsidian or Templater. Preserve the template's output shape:

   ```yaml
   ---
   tags:
     - "#Ideas"
   project: []
   date_created: YYYY-MM-DD HH:mm
   topics:
     - "[[Topic]]"
   ---
   # Title
   ---

   ## Appendix
   ---
   ```

7. Create the note at `<vault>/500 Zettelkasten/<Title>.md`. Insert all useful
   content before `## Appendix`.
8. Use `##` headings with a `---` divider immediately after each heading.
   Keep the note atomic, concise, and example-driven.
9. State the execution host for commands when it is not obvious. Include only
   facts and commands established in the conversation or source material; do
   not invent hostnames, credentials, paths, or network details.
10. Review the final note after writing. Verify the template structure,
    frontmatter, file path, heading dividers, title, links, command accuracy,
    and that the note answers the request without unnecessary detail.

## Completion Report

Report the note path and one sentence summarizing what the note preserves.
