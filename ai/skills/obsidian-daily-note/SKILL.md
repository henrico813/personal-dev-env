---
name: obsidian-daily-note
description: Adds a section or session summary under the "# Notes"
  heading of today's daily note in the default Obsidian vault, with
  nobs. It needs no Obsidian app and no screen, and creates a missing
  note from the vault's daily-note template. Use when the user asks
  to add something to the daily note, or says "put this in my notes"
  without naming another note. When the vault has no daily-note
  folder or its template cannot be filled, it asks the user which
  note to use. Never use it to read, search, or summarize a daily
  note. A new Zettelkasten note belongs to `obsidian-zettel`.
metadata:
  scope: personal
---

# Obsidian Daily Note

Add one approved section under `# Notes` in today's daily note, in
the default vault. Write with `nobs` only. Never use the Obsidian app
or its `obsidian` command, and never check for a screen, since nobs
needs neither.

`ai/AGENTS.md` names this skill as the one exception to its ban on
`nobs` commands that change a note. It uses these commands only:

- `nobs append-note` adds the section. It writes a new copy of the
  note and swaps it in, so a dropped link leaves the old note whole.
- `nobs append` creates today's note only when it is missing. A cut
  short write can then only hurt the new note, and the read-back
  catches it.
- `nobs read` checks each write afterwards.

## Privacy

- Pipe each `nobs read` into the `jq` checks below, which print only
  counts or a word. Never print a note body. Never run `nobs today`,
  since it prints the body.
- Read no note other than today's. The other daily notes hold private
  text from other days.
- When the user gives the text, such as "put this in my notes: water
  the ferns", write their words. When they ask for a session summary,
  summarize only this session's work.
- Leave out secrets, tokens, credentials, private URLs, other people's
  details, and health, mood, or personal-life content.
- Today only, default vault only.

## Block Rules

- Heading: a few words that name the topic, such as `Ferns` or
  `Session summary`. The user can change it at approval.
- The heading is one line of at most 80 characters, written as
  `## <heading>`. Start with a letter and use only letters, digits,
  spaces, and `-_.()/`, so it is safe inside the quoted commands below.
- Content: Markdown with no `,,` and no `<%`. The vault's quick-note
  script, which the user may run later, turns each `,,` into a blank
  line and runs `<%` as template code.
- No content line may start with `#` once spaces are trimmed, so the
  section stays one block under `# Notes`.

## Steps

Each tool call starts a new shell. Write each value a step prints
into later commands in place of its `<name>`. Remove the list indent
from each command before you run it. In step 6 the `SECTION_END` line
must then start at the beginning of its line, or the shell does not
see where the section ends.

1. Find today's note from the vault's daily-note settings:

   ```sh
   v=$(nobs vault path) && date +%F &&
     jq -r '[.folder // "", .format // ""] | @tsv' \
       "$v/.obsidian/daily-notes.json"
   ```

   It prints `<date>`, then the folder and the date format. Today's
   note is `<daily>`, which is `<folder>/<date>.md`. This skill
   cannot find today's note in three cases: a command fails, the
   folder is empty, or the format is neither empty nor `YYYY-MM-DD`.
   Then ask the user which note to use, and write nothing.
2. Choose the heading by the Block Rules, then check the note:

   ```sh
   nobs read "<daily>" | jq -r --arg h "## <heading>" '
     .body | split("\n") |
     "\(map(select(. == "# Notes")) | length) \(map(select(. == $h)) | length)"'
   ```

   - `1 0`: go to step 4.
   - `1 1`: an earlier run may have added this section. Add nothing,
     and tell the user. To add different text, ask for a new heading.
   - Another count: stop and report it. `append-note` needs exactly
     one `# Notes` line.
   - `note not found`: today's note is missing, and `nobs read`
     exits 4. Go to step 3.
3. Fill the daily-note template into a new note body. Obsidian's
   template plugins do not run without the app, so the command fills
   the tags they would. It prints one word:

   ```sh
   v=$(nobs vault path) &&
     t=$(jq -r '.template // ""' "$v/.obsidian/daily-notes.json") &&
     f="$v/${t%.md}.md" && n=~/.local/state/obsidian-daily-note/new.md &&
     mkdir -p "${n%/*}" &&
     if [ -z "$t" ]; then echo no-template
     elif [ ! -f "$f" ]; then echo template-missing; ls "${f%/*}" | head -20
     else sed -e "s/<% tp\.file\.creation_date() %>/$(date '+%F %H:%M')/g" \
         -e 's/<% tp\.file\.cursor() %>//g' -e 's/{{date}}/<date>/g' \
         -e 's/{{title}}/<date>/g' -e "s/{{time}}/$(date +%H:%M)/g" \
         "$f" > "$n" &&
       if grep -q -e '<%' -e '{{' "$n"; then echo template-code
       elif [ "$(grep -cx '# Notes' "$n")" != 1 ]; then echo notes-count
       else echo ok; fi
     fi
   ```

   - `ok`: go on.
   - `no-template` or `template-missing`: the settings name no
     template, or a file that is gone. Tell the user, and list the
     file names printed after the word. When they name a template,
     run this step again with the `t=$(jq ...)` line replaced by
     `t="<template>" &&`. Do not change the settings.
   - `template-code` or `notes-count`: the template has a tag this
     step does not fill, or no single `# Notes` line. Ask the user
     which note to use, and create nothing.
4. Ask for approval. Show the date, heading and content exactly as
   they will be written, in a fenced block. When step 3 ran, also
   say that today's note will be created from the template. Write
   nothing until the user approves that exact text. After any
   change, show it again.
5. Only when step 3 ran, create the note:

   ```sh
   n=~/.local/state/obsidian-daily-note/new.md
   nobs read "<daily>" > /dev/null 2>&1
   [ $? = 4 ] && nobs append "<daily>" < "$n" > /dev/null &&
     nobs read "<daily>" | jq -e --rawfile n "$n" '.body == $n' \
       > /dev/null && echo created && rm "$n"
   ```

   On `created`, go on. When it prints nothing, another device may
   have made the note. Run step 2 again, and go on only on `1 0`.
6. Run step 2 again, since the note may have changed. Then add the
   section and read it back, in one command:

   ```sh
   s=$(cat <<'SECTION_END'
   ## <heading>

   <content>
   SECTION_END
   ) && printf '%s\n' "$s" | nobs append-note "<daily>" &&
     nobs read "<daily>" | jq -r --arg s "$s" '
       .body | "\(split("\n") | map(select(. == "# Notes")) | length) \(split("\n\n" + $s + "\n") | length - 1)"'
   ```

   `append-note` puts the section right under the `# Notes` line.
   When a `---` line comes right after `# Notes`, the section goes
   below that `---` line. It prints a short JSON result. The
   check prints `1 1` when the section is in the note once and whole.

## Failure Handling

Never retry on your own. Never repair the note with another `nobs`
command, since a repair could overwrite what the user or sync changed.

- Steps 1 to 4 write nothing. Report the step and its message.
- Step 5 prints nothing and step 2 then fails: report that today's
  note may be incomplete, and ask the user to check it.
- `append-note` fails: report its message. The note is unchanged.
- The step 6 check does not print `1 1`: report that the result is
  not confirmed, and ask the user to check today's note.

## Completion Report

Report only the date and the result: added, added to a new note, not
confirmed, or the failed step and reason. Do not quote the daily note.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that you still print no note text. When the user corrects how
you pick the heading, write the section, run the checks or report the
result, follow [skill-fix](../skill-fix/SKILL.md) with the skill name
`obsidian-daily-note`, and offer once to fix the skill's text.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
