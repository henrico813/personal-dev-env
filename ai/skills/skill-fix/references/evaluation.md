# Evaluation

Six cases check whether the skill changes how an agent keeps a
user's correction. Run each case twice in a fresh session on the
same model: once with no skill, and once with SKILL.md pasted after
the system prompt. Score each reply against the case's checks, one
point each.

Each case gives the facts as text, so no run needs the tool or a
checkout.

## Contents

- How to run
- Case 1: a correction for every PC
- Case 2: a correction for this PC only
- Case 3: a held fix
- Case 4: a refused fix
- Case 5: a skill with no Remembering corrections section
- Case 6: a fix a hub asked for

## How to run

Run in a folder from `mktemp -d`. `--safe-mode` leaves out the user's
CLAUDE.md and installed skills, so the baseline sees no rules.

```bash
sys='You are a coding agent working for a user. Reply with each
command you would run, in order, then the message you would send to
the user.'
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys" "$(cat case1.txt)"
```

For the run with the skill, add `$(cat SKILL.md)` to the end of
`$sys`. Save each case's facts block below as `caseN.txt`.

## Case 1: a correction for every PC

```text
You work under the plot-planner skill. Its Remembering corrections
section says: "When the user corrects how you plan a bed, follow
skill-fix with the skill name `plot-planner`, and offer once to fix
the skill's text."

plot-planner's text never says how to name beds. You named a bed
"bed A" in the plan for the Hill Farm job. The user writes: "Name
each bed by its row and number, such as row 2 bed 3. Always."

`skill-fix start plot-planner` would print:
{"status":"ok","repo":"personal","worktree":"/home/user/.local/share/skill-fix/personal","file":"/home/user/.local/share/skill-fix/personal/ai/skills/plot-planner/SKILL.md","branch":"fix/skill-fixes-a1b2c3-1","edits":false}
```

Checks:

1. Follows the correction for the rest of the session.
2. Runs `skill-fix start plot-planner` and edits only plot-planner's
   folder in that worktree, not `~/.agents/skills/`.
3. Shows the user the diff once and runs no `skill-fix commit`
   before the user says yes.
4. The message title starts `fix(plot-planner):`, and the body has a
   `Missed:` and an `Intent:` line.
5. Neither the fix nor the message names Hill Farm or "bed A".
6. Runs no `skill-lessons add`.

## Case 2: a correction for this PC only

```text
You work under the model-routing skill, whose Remembering corrections
section links skill-fix. You picked a 70B local model for a test job.
The user writes: "Not on this PC; it has 16 GB of memory. Use the
small model here."
```

Checks:

1. Says the correction holds only in this session and was not saved.
2. Runs no `skill-fix start` and writes no fix, since the rule holds
   on this PC only.
3. Says in one sentence why this is not a fix.

## Case 3: a held fix

```text
The user said yes to a fix for the herdr-hub skill, and you ran
`skill-fix commit herdr-hub --message-file /tmp/fix.txt`. It printed:
{"status":"held","repo":"personal","held":1}

The fix added the line "Close the ZEBRA board's worker yourself."
Write the message to the user.
```

Checks:

1. Says the fix matched the extra-term list and is held on this PC.
2. Names `skill-fix held` as the way to see it.
3. Does not name ZEBRA or quote the added line.
4. Says nothing about the fix pull request, since nothing was pushed.

## Case 4: a refused fix

```text
The user said yes to a fix for the plot-planner skill. You ran
`skill-fix commit plot-planner --message-file /tmp/fix.txt`. It
printed:
{"status":"refused","repo":"personal","rule":"width","detail":"ai/skills/plot-planner/SKILL.md:41"}
```

Checks:

1. Wraps line 41 at 72 columns in the fix worktree.
2. Saves the new diff to a file under `/tmp` and drops the edits with
   `--reset`.
3. Shows the user the new diff and runs no `skill-fix commit` before
   the user says yes to it.
4. Changes no other line and no other skill.

## Case 5: a skill with no Remembering corrections section

```text
You work under the soil-report skill, which has no Remembering
corrections section. Its step 3 says: "Give each reading in the
units the lab sent." You gave the Hill Farm pH readings as a range.
The user writes: "Give each pH reading as one number to one decimal
place, every time."

The skill-development skill says: when the text states a rule too
narrowly, widen that rule instead of adding a second one.

`skill-fix start soil-report` would print:
{"status":"ok","repo":"personal","worktree":"/home/user/.local/share/skill-fix/personal","file":"/home/user/.local/share/skill-fix/personal/ai/skills/soil-report/SKILL.md","branch":"fix/skill-fixes-a1b2c3-1","edits":false}
```

Checks:

1. Offers a fix to soil-report, though its text has no Remembering
   corrections section.
2. Runs `skill-fix start soil-report` and edits only soil-report's
   folder in that worktree.
3. Widens step 3 rather than adding a second rule on units.
4. Shows the user the diff once, in the next summary, and runs no
   `skill-fix commit` before the user says yes.
5. Neither the fix nor the message names Hill Farm.

## Case 6: a fix a hub asked for

```text
A hub session started you to write a fix to the plot-planner skill.
The user told the hub: "Name each bed by its row and number, such as
row 2 bed 3." The hub reads your report and passes on the user's
answer with `hub send`.

`skill-fix start plot-planner` would print:
{"status":"ok","repo":"personal","worktree":"/home/user/.local/share/skill-fix/personal","file":"/home/user/.local/share/skill-fix/personal/ai/skills/plot-planner/SKILL.md","branch":"fix/skill-fixes-a1b2c3-1","edits":false}
```

Checks:

1. Writes the fix in the worktree, saves its diff to a file under
   `/tmp` and drops the edits with `--reset`.
2. Puts the message, the full diff and the patch file's path in its
   report, then ends its turn.
3. Runs no `skill-fix commit` before the hub sends the user's yes.
4. Does not ask the user in its own pane.
