# Evaluation

Three cases check whether the skill changes what an agent proposes
after a session. Run each case twice in a fresh session on the same
model: once with no skill, and once with SKILL.md pasted after the
system prompt. Score each reply against the case's checks, one point
each.

Each case gives the script's output and notes on the conversation as
text, so no run needs the database or any tool.

## Contents

- How to run
- Case 1: a costly session
- Case 2: the script fails
- Case 3: cheap cloud and local models

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
`$sys`. Save each case's facts block below as `caseN.txt`.

## Case 1: a costly session

```text
The user writes: "review this session for token usage and efficiency
improvements, skill lessons, and local model handoff". You saved the
review file already; write only the message.

The script printed:
session ses_77aa Tally P5 build
model claude-code/claude-opus-5-5[1m]
minutes open 88
tokens read 11,204,300 (cache read 10,990,120, cache write 210,760, uncached 3,420)
tokens written 60,118 (output 44,900, reasoning 15,218)
user messages 6, model steps 120, steps logged with zero tokens 6
peak context about 171,400 tokens at seq 990
compactions 1 (seq 612 auto)
skills loaded roadmap-run (seq 8), create-plan (seq 8), review-plan (seq 8), status-updates (seq 8)
subagents 2
  claude-code/claude-opus-5-5[1m] count 2 read 5,214,660 written 29,120
largest subagents by tokens read
  ses_77ab claude-code/claude-opus-5-5[1m] read 3,004,100 written 16,220 P5 planner
  ses_77ac claude-code/claude-opus-5-5[1m] read 2,210,560 written 12,900 P5 review
repeated commands 1
  5x cd ~/src/tally && npm test

Conversation notes:
- Each of the 5 npm test runs printed about 4,000 tokens; about 90
  later steps read each one again.
- The four skills loaded at seq 8 hold about 25,000 tokens.
- The user said: "Don't paste whole test logs into the chat; print
  the failures."
- AGENTS.md here belongs to the hub session tally-hub, not this one.
- The API cost column shows 0.
```

Checks:

1. At most three proposals.
2. Each proposal gives a saving marked measured or estimate.
3. Each proposal says where it would live: a skill PR, lesson,
   memory or rules file.
4. No proposal suggests skipping a skill the work needs.
5. Any AGENTS.md change goes to tally-hub, not edited here.
6. The test-log correction appears as a numbered lesson to pick.
7. No dollar amount.
8. The message ends by asking which numbers to apply, and says
   nothing was changed or applied.

## Case 2: the script fails

```text
The user writes: "review this session for token usage, lessons and
local model handoff". You saved the review file already; write only
the message.

The script printed to stderr and exited 1:
no OpenCode database at /home/user/.local/share/opencode/opencode.db

Conversation notes:
- You ran the test suite three times with the same command.
- The user corrected you once: "Use the repo's make test, not
  pytest directly."
```

Checks:

1. Says the review comes from the conversation alone, because the
   database was missing.
2. Gives no token totals, peak or step counts.
3. Any saving is marked estimate.
4. The make-test correction appears as a lesson to pick.
5. Changes nothing and asks which items to apply.

## Case 3: cheap cloud and local models

```text
The user writes: "which parts of this session could a local model
have done? Keep it short." You saved the review file already; write
only the message.

The script printed (subagent lines only):
subagents 3
  claude-code/claude-opus-5-5[1m] count 1 read 1,880,400 written 11,200
  openai/gpt-6-luna count 1 read 140,220 written 3,100
  goog/qwen3.8 count 1 read 28,400 written 760
largest subagents by tokens read
  ses_9c03 claude-code/claude-opus-5-5[1m] read 1,880,400 written 11,200 Review plan
  ses_9c01 openai/gpt-6-luna read 140,220 written 3,100 Cold read PR text
  ses_9c02 goog/qwen3.8 read 28,400 written 760 Run tests and linters

Conversation notes:
- goog/qwen3.8 runs on the user's own machine. openai/gpt-6-luna is a
  cheap cloud model.
- You also deleted two merged worktrees by hand, with a script that
  printed "done" at the end.
- roadmap-run's references/prompts.md has a Local model section on
  briefing a local model.
```

Checks:

1. Names the worktree cleanup as a job for the local model.
2. Does not call gpt-6-luna local.
3. Points to roadmap-run's Local model section instead of writing
   out a brief.
4. Says what would break if the plan review ran on the local model,
   and does not move it there while cloud models work.
5. Has a Fallback, an Efficiency and a Trials line, and the Trials
   line says none ran.
6. Fits a phone: about 20 lines or fewer.
