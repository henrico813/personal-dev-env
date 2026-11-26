# Evaluation

Four cases check whether the skill changes how an agent closes out a
session. Run each case twice in a fresh session on the same model:
once with no skill, and once with SKILL.md pasted after the system
prompt. Score each reply against the case's checks, one point each.

Each case gives the facts as text, so no run needs the database or
any tool.

## Contents

- How to run
- Case 1: the pick list
- Case 2: acting on the picks
- Case 3: a compacted session
- Case 4: the user picks nothing

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

## Case 1: the pick list

```text
The user types: "conclude-session". You ran the review script, saved
the review file, and a fresh agent checked it. Write only the message.

The script printed, in part:
session ses_77aa Tally P5 build
tokens read 11,204,300 (cache read 10,990,120, cache write 210,760, uncached 3,420)
list-price cost $4.06 (cache write $1.05, cache read $2.20, uncached $0.01, output $0.80)
rates per M tokens for claude-code/claude-opus-5-5: input $4, output $20, cache read $0.2, cache write $5
user messages 6, model steps 120, steps logged with zero tokens 6
peak context about 171,400 tokens at seq 990

The review file is ~/.local/state/pde/session-reviews/20261105-tally-p5.md.

The review found:
- Proposal: print only failing tests in auto-run's test step, a
  skill PR. Saves about $0.29 list price, 1.4M cache-read tokens
  (estimate: 4,000 tokens read again by 90 steps, 4 times, at
  $0.20 per million).
- Proposal: tell agents to print test failures only, in AGENTS.md.
  AGENTS.md here belongs to the hub session tally-hub.
- Fix: "Print test failures, never whole test logs" ->
  auto-run.

Conversation notes:
- The P6 build is not started. This session runs in Herdr.
- No pull request merged in this session.
```

Checks:

1. Each number names one item; no two lists both start at 1.
2. Offers a handoff of the P6 build and a status update.
3. Offers no cleanup.
4. The AGENTS.md change goes to tally-hub, not edited here.
5. Says nothing has changed and asks for numbers once.
6. Every number, title and path comes from the facts above.
7. Opens the numbers line with the $4.06 list price, and gives the
   proposal's saving in dollars.

## Case 2: acting on the picks

```text
You sent this list after a session review:
1. Print only failing tests in auto-run's test step (skill PR).
2. Fix: "Print test failures, never whole test logs" ->
   auto-run.
3. Hand the P6 build to a new Herdr session.
4. Status update.

The user replied: "1 2 4". The session's work is on branch
feat/tally-p5 in ~/src/tally. The skills live in
~/src/personal-dev-env/ai/skills and install to ~/.agents/skills.

List, in order, each action you take with its skill or command, then
write the last message to the user.
```

Checks:

1. Writes the auto-run fix with `skill-fix start auto-run`, shows
   its diff, and commits nothing until the user answers.
2. Edits auto-run only on a new branch from main in
   personal-dev-env, not on feat/tally-p5 or in ~/.agents/skills.
3. Opens one pull request with the pull-request skill and does not
   merge it.
4. Starts no handoff.
5. Runs `skill-fix sync`, then `skill-fix status`, before the last
   message.
6. The last message is the status update. It names the fix and the
   pull request, or its branch while approval waits, with no made-up
   number.

## Case 3: a compacted session

```text
The user types: "conclude-session". Before the review, list what you
will read and run, and nothing else.

The review script printed, in part:
user messages 41, model steps 380
compactions 2 (seq 512 auto, seq 1190 auto)
```

Checks:

1. Runs the session-review script for this session.
2. Reads only the user's messages from before seq 1190, cut short.
3. Reads no other session.
4. Writes the review itself; no subagent reads the conversation
   again.

## Case 4: the user picks nothing

```text
You sent a pick list with three proposals, one fix and a status
update. The user replied: "none. And stop proposing to move test
runs to a local model."
```

Checks:

1. Changes nothing: no fix, branch or pull request.
2. Offers once to add the rejected kind to session-review's Do not
   propose list as a fix, not to conclude-session.
3. Claims no rule is kept before the user agrees to save it.
4. Does not send the list again.
