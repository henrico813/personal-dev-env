---
name: screen-check
description: Runs a terminal program on a hidden tmux screen at phone
  size, presses its keys and reads the screen back as JSON, so an
  agent sees its own change before handing it over. Covers TUIs, the
  programs inside Herdr popups, and CLI programs. Use after changing
  anything a person sees in a terminal, instead of asking the user to
  test it. Not for web pages or browsers.
---

# Screen Check

The user reads every screen on a phone, through an SSH terminal about
40 columns wide. A change that looks fine on a wide monitor can wrap,
cut off a row or hide a key hint there. This skill has the agent run
the changed program on a screen of that size, look at what it shows
and fix it, so the user does not have to test each change.

`scripts/screen.py` runs the program inside its own tmux server, with
no window. Nothing appears in the user's Herdr workspace, and no
command reaches another session's panes.

## Inputs

- The change, and the screens it touches.
- What each screen must show after the change, as text a person would
  see, such as a new menu row `Archive`.
- A command that runs the changed code from the worktree, such as
  `go run ./cmd/glimmer`, `cargo run --quiet --` or `uv run app`.
  The installed copy on PATH does not have the change yet.

## Check a change

Each shell call starts fresh, so begin each one with
`S=~/.agents/skills/screen-check/scripts/screen.py`.

1. Start the program on a new screen. Pick a name of lowercase
   letters, digits and hyphens, such as the branch's words. The
   default size, 40x16, is the user's phone:

   ```bash
   python3 $S start menu-archive --cwd "$PWD" -- go run ./cmd/app
   ```

2. Wait for text from the first screen before you press anything:

   ```bash
   python3 $S wait menu-archive "Inbox"
   ```

   `wait` prints the screen as JSON. It exits 1 when the text does
   not appear within `--timeout` seconds, default 10, or when the
   program exits first. Never use `sleep` instead.
3. Reach the screen you changed. `type` sends text as written.
   `keys` sends tmux key names, such as `Enter`, `Escape`, `Tab`,
   `Up`, `Down`, `BSpace` and `C-c`. Run `wait` after each step:

   ```bash
   python3 $S keys menu-archive Down Down Enter
   python3 $S wait menu-archive "Archive"
   ```

4. Read the whole screen with `show`, not just the text you waited
   for. Look for each of these:
   - A label or key hint cut at column 40, or wrapped onto the next
     row.
   - Rows that no longer fit in 16 lines, so the last ones are gone.
   - Text drawn over other text, or a border that does not close.
   - The cursor, in `cursor`, away from where typing goes.

   Add `--ansi` when the change is about color or highlighting, such
   as which row is selected. Each line then keeps its escape codes.
5. When something is wrong, fix the code, run `stop`, and start
   again from step 1. Repeat until the screen is right.
6. Check other sizes only when the change is about them, with
   `--size 80x24` on a second screen.
7. Stop every screen you started, also after a failure:

   ```bash
   python3 $S stop menu-archive
   ```

For CLI output, `start` the command the same way and `wait` for its
last line. The screen keeps the output after the command exits. The
`exit_code` field in the JSON holds the command's exit code.

For a Herdr popup, start the program the popup runs, at the popup's
size. Herdr's own frame, sidebar and bars are not on the screen. Say
in your report that they were not checked.

Some projects already have screen tests, such as Bubble Tea's
`teatest` or Ratatui's `TestBackend`. In that case, also add or
update one for the changed screen, by the `behavior-focused-testing`
skill. A screen check shows what a person sees. A test keeps the
screen that way.

## Output format

Keep the report to the length the `status-updates` skill sets.
Report each screen you checked with these parts:

- The command, its size, and the keys that reached the screen.
- For one screen only, its final `lines` inline in a code block, so
  the user sees it without running anything. Put any other screens
  where `status-updates` says detail goes.
- What you checked on it, and what you fixed after a check failed.
- What the check could not show, such as Herdr's frame or how the
  phone's keyboard covers the bottom rows.

## Guardrails

- While checking a screen, send keys only to a screen you started
  with `screen.py`. Never type into a tmux session, a Herdr pane or
  another agent's pane, such as with `herdr pane send-keys`.
- Give the program made-up data in a scratch folder. Never point it at
  real accounts, pull requests or approval requests.
- Ask the user to look only at what a screen cannot show, such as how
  a key feels on the phone.

## When a tool is missing

- When `screen.py` exits 2 with `tmux is not installed`, or the script
  is not installed, say in your report that the screen check did not
  run. For a CLI, run the command with `COLUMNS=40` and read its
  output, and say that the output was not checked on a screen.
- When the program does not start, `wait` exits 1 and prints the
  screen with the program's error. Fix the command and start again.

## Final checks

- Each changed screen was checked at 40x16.
- You read every line of the final screen, not only the text you
  waited for.
- `python3 $S show <name>` exits 2 for every screen you started, so
  none is still running.
- The report shows one final screen in a code block and links any
  other screens.

## References

- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you check a screen, follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`screen-check`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
