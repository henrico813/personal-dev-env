---
name: daily-shutdown
description: Runs the user's end-of-day shutdown. Reads the day's
  notes, drafts tomorrow's tasks, evening ideas, what was learned and
  any handoff notes, waits for the user to approve the draft's exact
  text, then adds each block to the note this PC's settings name, one
  at a time, through obsidian-daily-note. Use when the user asks to
  do the shutdown or to close out the day. Do not use it to close
  out one session; conclude-session owns that. Do not use it to add
  one section to the daily note; obsidian-daily-note owns that.
metadata:
  pde-workflow: "true"
  scope: personal
---

# Daily Shutdown

At the end of a day the user asks for "the shutdown". The
shutdown is a short note on what comes next and what the day taught.
This skill drafts it from the day's notes, has the user approve the
exact text, and then writes it. It owns the note's blocks and the
order of approval and writing. The `obsidian-daily-note` skill does
each write. The Usage block's numbers come from the review files
that `session-review` saved.

## Inputs

- This PC's settings, the `daily_shutdown` object in
  `~/.config/pde/config.json`. The user adds it by hand. When
  `pde-installer` rewrites the file, it keeps keys it does not set.

  ```json
  {
    "daily_shutdown": {
      "destination": "daily-note",
      "sources": [
        "~/.local/state/herdr-hub.*/notes.md",
        "~/notes/board-summary.md"
      ]
    }
  }
  ```

  - `destination`: where the approved blocks go. `daily-note` means
    today's daily note in the default vault, written through
    `obsidian-daily-note`. No other value works yet. Leave out
    `destination` to only draft.
  - `sources`: the files to read, as paths or `*` patterns. A path
    may start with `~`. When `sources` is missing or empty, read no
    source file and say so in step 4's message.
- The drafts in `~/.local/state/pde/shutdown/`, one per day.
- Today's session reviews in `~/.local/state/pde/session-reviews/`,
  for the usage numbers. These are the files whose names start with
  today's `date +%Y%m%d`.

## The draft

Each draft is `~/.local/state/pde/shutdown/<date>.md`, with `<date>`
from `date +%Y%m%d`. It starts with two status lines, then the
blocks. Each block is a `## <heading>` line, a blank line, then its
content:

```markdown
Approved: no
Written: none

## Tomorrow

1. Review the retry pull request. Next step: read the reviewer's
   comment on the timeout test.
```

- `Approved:` turns to `yes` when the user approves the text.
- `Written:` lists, in order, the heading of each block that
  `obsidian-daily-note` added, such as `Written: Tomorrow, Handoff`.

The status lines tell a later run what reached the note, since this
skill never reads the note itself.

## Steps

### 1. Read the settings

```bash
jq '.daily_shutdown // empty' ~/.config/pde/config.json
```

When `destination` is missing or is not `daily-note`, this run only
drafts. Step 4 then asks no question, and nothing is written.

### 2. Read the day's notes

1. Among the drafts before today that say `Approved: yes`, read
   the one with the latest date in its file name. Skip drafts the
   user never approved, since they may hold tasks the user turned
   down. Its Tomorrow list shows what today was meant to do. Read
   this draft in place of a daily note, as Guardrails say.
2. Read each file that a `sources` entry matches and that changed
   today. Run this once per entry, with the entry in place of
   `<entry>`:

   ```bash
   bash -O nullglob -c 'for f in <entry>; do
     if [ -e "$f" ] &&
       [ -n "$(find "$f" -maxdepth 0 -type f -newermt "$(date +%F)")" ]
     then echo "$f"; fi
   done'
   ```

   Quote a space inside an entry, but never `~` or `*`, so bash
   still expands them, such as `~/"My notes"/*.md`. `nullglob` drops
   a pattern that matches nothing, where zsh would stop with an
   error. When an entry prints no file, say so in step 4's
   message. Read no other source file, since the settings list the
   ones the user chose.
3. From each of today's session reviews, take its title, the words
   after `# Session review: ` on its first line, such as `Upload fix`.
   Take three numbers from its Numbers section: tokens read, peak
   context and model steps. Copy each number as written.
   Never run `session-review`'s `session_stats.py`, and never count
   tokens yourself. A
   second count would not match the review.

### 3. Write the draft

Load `obsidian-daily-note` now, since its Block Rules and Privacy
section shape what the blocks may hold.

When today's draft exists, an earlier run today wrote it. Keep its
text and status lines, since the user may have changed the text.
Add only what changed in the sources since then, and only to blocks
that `Written:` does not list. Any change sets `Approved:` back to
`no`. When no draft exists for today, write a new one with
`Approved: no` and `Written: none`.

Keep the draft out of the vault, so sync never copies text the user
has not approved. Write these blocks in this order:

1. Tomorrow: a numbered list of tasks. Give each task its next
   step, such as the command to run or the file to open first. Carry
   over each task from the last approved draft's Tomorrow list that
   today's notes do not show as done.
2. Evening ideas: ideas from today's notes that the user may think
   over after work. Leave it empty when there are none.
3. Learned today: one bullet per thing learned. End each bullet with
   its source, such as a pull request, a doc or a command's output.
4. Handoff: notes for whoever picks the work up next, such as a
   session that waits on a review. Leave it empty when there are
   none.
5. Usage: one line per session review from step 2, in this form,
   with no other words:

   ```text
   <title>: tokens read <n>, peak context about <n>, model steps <n>
   ```

   Leave out the review's other details, such as the parts in
   parentheses. Leave the block empty when today has no review.

Keep each block within the Block Rules of `obsidian-daily-note`.

### 4. Get approval

Ask in every run, even when the draft already says `Approved: yes`,
since the user may want a change before more blocks reach the note.

Open the message with the facts that change what the user expects:
that this run only drafts and why, each `sources` entry that printed
no file, and a missing approved draft before today. Then show the
whole draft in a fenced block, exactly as each block will be
written. Name the empty blocks that will be skipped, the blocks
`Written:` already lists, and the draft's path.

When this run only drafts, ask nothing and end the skill. Otherwise
end with one question in the short form of the `status-updates`
skill, with lettered options, such as whether to write the named
blocks.

When the user asks for a change, edit the draft file, set
`Approved:` to `no`, and show the whole draft again. A block that
`Written:` lists is in the note already, so a change to it does not
reach the note. Tell the user to edit the note by hand, since this
skill never edits a note. When the user approves, set
`Approved: yes`.

### 5. Write each block

1. For each block that has content and that `Written:` does not
   list, in the order of step 3, run `obsidian-daily-note` once with
   the block's heading and content. Skip each empty block.
2. That skill owns the write, so its own approval step still runs
   for each block. Pass its question to the user, and let the user
   answer it. Its heading and content must be the draft's text, word
   for word. When they differ, stop and tell the user.
3. Wait for each run's report before you start the next one. When
   the report says the block was added, add its heading to
   `Written:`. When it says anything else, write no later block,
   since the blocks must stay in order in the note.

## Output format

A full draft, with one empty block, before approval:

```markdown
Approved: no
Written: none

## Tomorrow

1. Review the retry pull request. Next step: read the reviewer's
   comment on the timeout test.
2. Finish the backup script. Next step: run it with `--dry-run`.

## Evening ideas

## Learned today

- `find -newermt` takes a date, so it can pick files changed today.
  Source: GNU findutils manual.

## Handoff

- The docs session waits on a review of its pull request.

## Usage

Docs build: tokens read 9,412,880, peak context about 162,310, model steps 96
```

The last message follows the `status-updates` skill. It names each
block written, each block skipped as empty, each block not written
and why, the draft's `Written:` line, and the draft's path.

## Guardrails

- Write only through `obsidian-daily-note`. Never edit a note file
  yourself, since that skill checks the note and reads each write
  back.
- Never read a daily note yourself, today's or an earlier one, and
  never list the daily-note folder. Read the last approved draft
  instead, since it holds the text the skill wrote.
- Write only to the `destination` the settings name. When it is
  missing, write nothing, even when a vault and its daily note exist.
- Copy usage numbers from the session reviews. Never compute them.
- Keep each block within the Privacy section of
  `obsidian-daily-note`.

## When a tool is missing

- `~/.config/pde/config.json` or `jq` is missing: treat the settings
  as empty, as step 1 says.
- `obsidian-daily-note` is not installed: write nothing. Give the
  user the draft's path.
- No approved draft before today exists: draft from today's sources
  alone, and say so in step 4's message.

## Final checks

- The draft sits in `~/.local/state/pde/shutdown/`, not in the vault.
- The user approved the exact text before any write.
- Each non-empty block not yet written went through its own
  `obsidian-daily-note` run, in order, and no block followed a
  failed one.
- `Written:` lists each block that was added.
- You read no daily note yourself.
- The usage numbers match the session reviews they came from.

## References

- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that you still never read a daily note. When the user corrects
how you draft, approve or write the shutdown, follow
[skill-fix](../skill-fix/SKILL.md) with the skill name
`daily-shutdown`, and offer once to fix the skill's text.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
