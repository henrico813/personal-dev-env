# Evaluation

Three cases check whether the skill changes what an agent writes. Run
each case twice in a fresh session on the same model: once with no
skill, and once with SKILL.md pasted after the system prompt. Score
each reply against the case's checks, one point each.

## Contents

- How to run
- Case 1: a messy progress update
- Case 2: a recap after time away
- Case 3: a short answer to "what is X?"

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

## Case 1: a messy progress update

```text
Project: Lantern, a command-line tool that backs up a family photo
library to a home file server. You are building its roadmap as a
series of pull requests. The user last read an update from you
yesterday.

Your notes:
- P1 #41 hash-skip: pushed, CI green (14 tests pass). Approval
  request filed 2h ago; requests expire 4h after filing. Needs the
  user's approval. 212 changed lines.
- P2 #42 resume: built. Review found a half-copied file counted as
  done; fixed in a second commit; the new test fails without the fix
  (checked). Comment review queued: worktree
  ~/src/lantern/wt/feat-resume, the user runs `lrev 42` there.
- P3 #43 progress bar: plan in fresh review, nothing for the user.
- Yesterday I said P1's tests cover Windows paths. Wrong: no Windows
  test exists; only Linux paths ran.
- Picked config key name `resume_dir` over `resume_path` (minor).
- Old branch tmp/hash-spike could be deleted; I mentioned it
  yesterday and the user did not answer.
- batch 2 (P4-P6, scheduled backups) not started.

Write your status update to the user now.
```

Checks:

1. Each pull request is named by what it does, not only "P1" or
   "#41".
2. The update opens with what changed, then lists what the user must
   do, in order.
3. The approval says it expires in about two hours.
4. The comment review gives the worktree path and `lrev 42`.
5. The Windows claim is corrected plainly.
6. The `resume_dir` choice is stated as decided, not asked.
7. The tmp/hash-spike branch is left out.
8. "batch 2" is explained or left out.
9. No filler words, hedges, closing offer or emojis.
10. Fits a phone: about 20 lines or fewer, no table over three
    columns.

## Case 2: a recap after time away

```text
Project: Tally, a small web app that tracks a household budget. The
user writes: "back after two days, where are we?"

Your notes from the last two days:
- #12 CSV import from the bank: merged Tuesday.
- #13 category rules (auto-tag "GROCERY*" as food): in progress. 31
  of 32 tests pass; test_feb29_rule fails because the rule engine
  parses dates without leap years. Fix not written yet.
- #14 monthly summary email: pushed, waiting on the user's approval
  in the approval app; the request expires tonight at 22:00. Branch
  feat/monthly-email, worktree ~/code/tally/wt/monthly-email.
- Open question Q2 from Tuesday: when an imported row duplicates an
  existing one, skip it silently or flag it for review? Blocks #13's
  last step. You lean toward flagging.
- Load test of the import with 5,000 rows: 3.1 s. 50,000 rows not
  tested.
- Earlier optional idea: dark mode. The user did not reply.

Write the recap.
```

Checks:

1. The recap is grouped as done, in progress, waiting on you.
2. Each pull request is named by what it does.
3. The duplicate-rows question is asked in full, with what each
   answer leads to and a recommendation, without relying on "Q2".
4. The approval gives its deadline and where to approve.
5. The failing leap-day test is reported plainly.
6. The untested 50,000-row case is stated.
7. Dark mode is left out.
8. No filler words, hedges, closing offer or emojis.
9. Fits a phone: about 20 lines or fewer, no table over three
   columns.

## Case 3: a short answer to "what is X?"

```text
Project: Lantern, a command-line tool that backs up a family photo
library to a home file server. Earlier today you wrote to the user:
"batch 2 starts after P3 lands." The user replies: "what is batch
2?"

Your notes: batch 2 is the second group of roadmap pull requests, P4
to P6. They add scheduled backups: P4 a schedule setting, P5 a
background timer, P6 a warning for a missed backup. None started.

Answer the user.
```

Checks:

1. The answer is one or two sentences.
2. It says what batch 2 does for the user in plain words.
3. It does not lean on "P4" to "P6" or "P3" without saying what they
   are.
4. No "Great question", restated question or closing offer.
