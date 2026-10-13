---
name: obsidian-daily-note
description: Use when adding a section or session summary to today's journal daily note in the main Obsidian vault. Never use it to read, search, or summarize a daily note.
---

# Obsidian Daily Note

Add one approved section to today's daily note in the main vault. Write a
quick note in the format `_createQuickNote()` produces, then run the vault's
`_appendCurrQuickNotes()` Templater command inside the Obsidian app. Do not
change vault templates or scripts.

## Privacy

- Run no `obsidian` command other than those listed in this skill. Their
  results are only a path, a boolean, a count, a status word, or the heading.
  For example, never run `read`, `daily:read`, `search`, `daily`, or an
  `eval` that returns note text.
- Never read, list, search, or open anything under `300 Journal/`, and never
  edit a journal file directly.
- Summarize only this session's work. Leave out secrets, tokens,
  credentials, private URLs, other people's details, and health, mood, or
  personal-life content.
- Today only, main vault only.

## Block Rules

The template appends the block as `## <heading>` and the content under the
daily note's `# Notes` section.

- Heading: one line of at most 80 characters with no leading or trailing
  space. Start with a letter and use only letters, digits, spaces, and
  `-_.()/`. It must not contain `task` in any case (the template moves it to
  `# Tasks`) and must not be `true`, `false`, `null`, `yes`, `no`, `on`, or
  `off` in any case. These rules keep the unquoted YAML value a string and
  make it safe inside the quoted commands below.
- Content: Markdown with no `,,` (the template turns every `,,` in the daily
  note into a blank line), no `<%` (Templater would run it as template code
  if file-creation triggers are enabled), and no line whose trimmed text
  starts with `#`. End with one newline.

## Commands

Each agent tool call starts a new shell, so define this helper in the same
shell command as its uses. It selects the vault and drops two noise lines:

```sh
obsidian_main() {
  obsidian vault="Main Vault" "$@" 2>&1 |
    grep -v -e 'Loading updated app package' -e 'installer is out of date'
}
```

Shell variables do not survive between tool calls. `<heading>` is the
heading chosen before step 1; record `<vault>` in step 2, `<daily>` and
`<parent>` in step 3, and `<name>` and `<quick>` in step 5. Substitute them
literally in every later command. Keep paths and the heading inside the
quotes shown; paths with spaces stay inside one quoted value.

`eval` prints `=> <value>`; other `obsidian_main` commands print the bare
value. A single-shot `obsidian_main` check passes only when it prints the
exact expected output; any other output (an `Error:` line, `(no output)`, or
nothing) means stop and report. Shell checks state their own output. A poll
passes when it prints its target value; other values while polling are
expected, and only a timeout fails.

The CLI drops the result of any eval that takes more than about 10 ms, so
evals that read the daily note or change tabs only start the work. They
store a count or status word in `window.pdevDailyNote`, never note text.
Read a stored `<key>` with this instant eval; it prints `=> none` when the
value is missing or belongs to another run:

```sh
obsidian_main eval code='(r => r?.run === "<name>" ? r.<key> ?? "none" : "none")(window.pdevDailyNote)'
```

From step 6 on, end every run, including a stopped one, with
`obsidian_main eval code='delete window.pdevDailyNote'`; ignore its output.

## Pre-checks

Choose the heading first; step 6 needs it. Run these checks before showing
the full draft.

1. Run
   `[ -n "$DISPLAY$WAYLAND_DISPLAY" ] && echo "display ok" || echo "no display"`.
   It passes on `display ok`. On `no display`, stop and report that
   Obsidian needs a display.
2. Record the output of `pde vault path main` as `<vault>`.
   `obsidian_main vault info=path` must print exactly `<vault>`. Per the
   Obsidian CLI docs, the first command launches Obsidian if it is not
   running.
3. `obsidian_main daily:path` must print exactly
   `300 Journal/301 Current Entries/<YYYY-MM-DD>.md` with today's date from
   `date +%F`. Record it as `<daily>` and its basename without `.md` as
   `<parent>`.
4. Run `obsidian_main eval code='!!app.vault.getFileByPath("<daily>")'`
   every 2 seconds until it prints `=> true`, for up to 60 seconds. If it
   never does, stop and report that today's daily note does not exist. Do
   not create it; never run `obsidian daily`.
5. Run `date +%Y-%m-%dT%H-%M-%S` once and record `<name>` as
   `Quick Note <timestamp>` and `<quick>` as `500 Zettelkasten/<name>.md`.
   Never run `date` again for this run. `<name>` is also the run ID.
   `test -e "<vault>/<quick>" && echo exists || echo absent` must print
   `absent`, and
   `obsidian_main eval code='app.vault.getMarkdownFiles().some(f => f.basename === "<name>")'`
   must print `=> false`, because the template finds the note by basename.
6. Start the daily note counts. This eval replaces any earlier
   `window.pdevDailyNote` and defines `count()` for reuse; ignore its
   output:

   ```sh
   obsidian_main eval code='window.pdevDailyNote = { run: "<name>", count() { this.counts = "pending"; Promise.resolve().then(() => app.vault.read(app.vault.getFileByPath("<daily>"))).then(d => { const t = d.split("\n").map(x => x.trim()), i = t.indexOf("# Notes"); this.counts = [t.filter(x => x === "# Notes").length, i >= 0 && i + 1 < t.length ? 1 : 0, d.split(",,").length - 1, t.filter(x => x === "## <heading>").length].join(" "); }, () => { this.counts = "error"; }); } }; window.pdevDailyNote.count()'
   ```

   Read `counts` every 2 seconds, for up to 30 seconds, until it is not
   `=> pending`. It must print `=> 1 1 0 0`; otherwise stop. The four
   numbers are:
   - `# Notes` lines: must be 1.
   - A line after `# Notes`, because the template appends to that line:
     must be 1.
   - `,,` matches: must be 0, because the template would rewrite them.
   - `## <heading>` lines: must be 0. If it is 1, the heading already
     exists, possibly from an earlier unconfirmed run: do not add the same
     content again. Ask for a new heading only for different content.

## Approval

Show the target date, heading, and content exactly as they will be written,
in a fenced block. Write nothing until the user approves that exact text.
Any later change needs new approval and new step 6 counts. Tell the user
that a new Obsidian tab will open on the quick note and an empty tab may
remain. Ask them not to edit today's daily note until the report; the
template reads the note and then overwrites it.

## Append

7. Rerun step 3 and both step 5 checks. Then restart the counts with
   `obsidian_main eval code='window.pdevDailyNote?.run === "<name>" && window.pdevDailyNote.count()'`,
   ignore its output, and read `counts` as in step 6. All must give the
   same results as before; a different daily path means the date changed.
8. Write `<vault>/<quick>` with exactly five frontmatter lines; the content
   starts on line 6:

   ```markdown
   ---
   parent:
     - "[[<parent>]]"
   heading: <heading>
   ---
   <content>
   ```

9. Run the eval below every 2 seconds until it prints `=> <heading>`, for
   up to 30 seconds. `=> missing` and `=> waiting` are expected while
   Obsidian indexes the note; only a timeout fails.

   ```sh
   obsidian_main eval code='(f => f ? app.metadataCache.getFileCache(f)?.frontmatter?.heading ?? "waiting" : "missing")(app.vault.getFileByPath("<quick>"))'
   ```

10. Open the quick note in a new tab, check that it is the active editor,
    and run the template, all in one eval. This eval only opens a tab and
    runs the command. It stores the result in `status`; ignore its output:

    ```sh
    obsidian_main eval code='(async () => { const r = window.pdevDailyNote, q = "<quick>"; if (r?.run !== "<name>") return; r.status = "pending"; await app.workspace.getLeaf(true).openFile(app.vault.getFileByPath(q), { active: true }); if (app.workspace.activeEditor?.file?.path !== q) { r.status = "not-active"; return; } r.status = app.commands.executeCommandById("templater-obsidian:000 Meta/001 Templates/_appendCurrQuickNotes().md") ? "fired" : "no-command"; })()'
    ```

11. Read `status` every 2 seconds until it prints `=> fired`,
    `=> not-active`, or `=> no-command`, for up to 10 seconds. The step 10
    eval sets `pending` before its first `await`, so `=> none` for the
    whole poll means it never ran. Decide by this status, not by step 10
    output: an `Error:` there may come from the command itself, so
    `pending` still goes to step 12.
12. On `=> fired`, or when step 11 times out on `=> pending`, run
    `obsidian_main eval code='!!app.vault.getFileByPath("<quick>")'` every 2
    seconds until it prints `=> false`, for up to 30 seconds. The command
    returns before the template finishes writing over the NAS, and a
    pending step 10 may still fire it. If Templater recreates the note as
    an empty file, this poll runs the full 30 seconds; the rules below
    treat that case as success.

## Verification

After step 12, restart the counts as in step 7 and read `counts`. The run
succeeded when the last number is 1 and either:

- the step 12 poll printed `=> false` and the step 5 file check prints
  `absent`, or
- `stat -c %s "<vault>/<quick>"` prints `0`. Templater may recreate the
  deleted note as an empty file. Delete it only when it is exactly 0 bytes.

## Failure Handling

Never retry automatically. "Heading count" below is the last number of
`counts`.

- Steps 1-7: stop and report; nothing was written.
- Steps 8-9: delete `<vault>/<quick>`, confirm it is gone, then report.
- Step 11 prints `=> not-active` or `=> no-command`, or prints `=> none`
  for the whole poll: the command did not run. Delete `<vault>/<quick>`,
  confirm it is gone, then report that nothing was written.
- The quick note is not empty after step 12 and the heading count is 1:
  report that the section was added to today's note but the quick note was
  not deleted, and ask the user to remove `<quick>`. Do not suggest a retry.
- The quick note is not empty after step 12 and the heading count is 0:
  report the result as not confirmed, because NAS writes can outlast the
  poll. Ask the user to check today's note in Obsidian and to delete
  `<quick>` before any retry.
- The quick note is gone but the heading count is not 1: report the count.
  Do not edit the daily note.
- Any other result, including `counts` that stay `pending`, print `none`,
  or print `error`: report the step, the quick note check, and the heading
  count if known. Delete nothing.

## Completion Report

Report only the date and the result: added, not confirmed, or the failed
step and reason. Do not quote the daily note.
