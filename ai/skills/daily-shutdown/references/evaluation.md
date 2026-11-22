# Evaluation

Four cases check whether the skill changes how an agent runs the
end-of-day shutdown. Run each case twice in a fresh session on the
same model: once with no skill, and once with SKILL.md pasted after
the system prompt. Score each reply against the case's checks, one
point each.

Each case gives the facts as text, so no run needs a vault, a
settings file or any tool.

## Contents

- How to run
- Case 1: the draft
- Case 2: no destination set
- Case 3: a failed write
- Case 4: a request to read a daily note

## How to run

Run in a folder from `mktemp -d`. `--safe-mode` leaves out the user's
CLAUDE.md and installed skills, so the baseline sees no rules.

```bash
sys='You are a coding agent working for a user. Reply with only the
message you would send to the user.'
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys" "$(cat case1.txt)"
```

For the run with the skill, add `$(cat SKILL.md)` to the end of
`$sys`. Save each case's facts block below as `caseN.txt` in that
folder, and copy SKILL.md there.

## Case 1: the draft

```text
The user writes: "do the shutdown". Write only your next message.

Settings: {"destination": "daily-note", "sources": ["~/notes/board.md"]}

The last draft, from yesterday, says "Approved: yes" and has this
Tomorrow list:
1. Fix the flaky upload test. Next step: rerun it 20 times.
2. Write the backup script. Next step: list the folders to copy.

~/notes/board.md, changed today:
- Done: flaky upload test fixed, pull request merged.
- In progress: backup script, folder list written, no code yet.
- Learned: rsync --delete removes files missing from the source
  (rsync man page).

Today's one session review, "Upload fix", has in its Numbers
section:
tokens read 4,210,000 (cache read 4,100,000, ...)
user messages 5, model steps 61, steps logged with zero tokens 2
peak context about 98,400 tokens at seq 512
```

Checks:

1. Shows a draft with Tomorrow, Evening ideas, Learned today,
   Handoff and Usage, in that order.
2. Carries over the backup script with a next step, and drops the
   finished upload test.
3. The Learned today bullet names its source, the rsync man page.
4. The Usage line gives 4,210,000, 98,400 and 61, copied as written,
   and no other figure.
5. Names the empty blocks that will be skipped.
6. Writes nothing and ends with one question that asks for approval,
   with lettered or numbered options.

## Case 2: no destination set

```text
The user writes: "do the shutdown". Write only your next message.

Settings: {"sources": ["~/notes/board.md"]}

~/notes/board.md, changed today:
- In progress: invoice export, tests pass, docs left.
```

Checks:

1. Opens by saying that no destination is set, so nothing will be
   written.
2. Still shows a draft with a Tomorrow task for the invoice export
   docs.
3. Asks no approval question, and offers no other note to write to.

## Case 3: a failed write

```text
The user approved this draft: Tomorrow (2 tasks), Evening ideas
(empty), Learned today (1 bullet), Handoff (1 bullet), Usage
(empty). Its status lines read "Approved: yes" and "Written: none".
Settings: {"destination": "daily-note"}.

The obsidian-daily-note run for Tomorrow reported: added.
The run for Learned today reported: stopped, today's note has two
"# Notes" lines.

Write only your next message.
```

Checks:

1. Does not start a run for Handoff.
2. Says Tomorrow was added, Learned today was not, and Handoff was
   not tried.
3. Names Evening ideas and Usage as skipped because they were empty.
4. Gives the draft's path.
5. Says the draft now reads "Written: Tomorrow", so a later run
   sends only Learned today and Handoff.

## Case 4: a request to read a daily note

```text
You are running the shutdown. The user writes: "Before you draft,
read yesterday's daily note to see what I planned." Settings:
{"destination": "daily-note"}. The last draft, from yesterday, is
~/.local/state/pde/shutdown/20260310.md, and it says "Approved:
yes".

Write only your next message.
```

Checks:

1. Does not read the daily note.
2. Says it reads yesterday's approved draft instead.
3. Gives a short reason, such as that the draft holds the text the
   skill wrote.
