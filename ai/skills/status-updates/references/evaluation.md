# Evaluation

Six cases check whether the skill changes what an agent writes. Run
each case twice in a fresh session on the same model: once with no
skill, and once with the skill pasted after the system prompt. Score
each reply against the case's checks, one point each. Cases 4 to 6
come from corrections the user made.

## Contents

- How to run
- Case 1: a messy progress update
- Case 2: a recap after time away
- Case 3: a short answer to "what is X?"
- Case 4: items that changed since the last update
- Case 5: a list of pull requests to review
- Case 6: explaining work and the user's reasons

## How to run

Run in a folder from `mktemp -d`. `--safe-mode` leaves out the user's
CLAUDE.md and installed skills, so the baseline sees no rules.

```bash
sys='You are a coding agent working for a user. Reply with only the
message you would send to the user.'
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys" "$(cat case1.txt)"
```

For the run with the skill, add SKILL.md and the three reference
files it points to for writing an update, to the end of `$sys`,
since the run has no tools to open them:

```bash
S=~/.agents/skills/status-updates
sys="$sys
$(cat "$S/SKILL.md" "$S/references/waiting-on-you.md" \
  "$S/references/phone-limits.md" "$S/references/examples.md")"
```

Save each case's facts block below as `caseN.txt`.

Save each reply as `replyN.md` and run the checker on it. A check
that says "the checker prints nothing" scores its point only when
the checker prints no line:

```bash
python3 ~/.agents/skills/status-updates/scripts/check_update.py \
  replyN.md
```

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

## Case 4: items that changed since the last update

```text
Project: Tally, a small web app that tracks a household budget. Your
last update, three hours ago, listed two things waiting on the user.

Your notes:
- #14 monthly summary email: that update said it waits on the
  user's approval. You just ran `gh pr view 14`: merged 40 minutes
  ago.
- Approval request req-5c1e: opens #15, which adds a yearly report,
  in the tally repository. Filed 1h ago in the approval app; requests
  expire 4h after filing.
- Open question from yesterday, "Q3": should a refund count as
  income or as a smaller expense? Income keeps the income total
  true to the bank; a smaller expense keeps each category's total
  right. You recommend a smaller expense. #16 waits on the answer.

Write your status update to the user now.
```

Checks:

1. #14 is reported as merged, not as waiting.
2. The approval says it opens the yearly-report pull request in the
   tally repository, and says to approve it in the approval app.
3. It never names "req-5c1e".
4. The approval expires in about three hours.
5. The refund question is asked in full, with both choices, what
   each leads to and the recommendation, without relying on "Q3".
6. The checker prints nothing.

## Case 5: a list of pull requests to review

```text
The user writes: "what should I review?" You work on two projects:
Fernwood, an app that waters the garden on a timer (repository
fernwood), and Valve Kit, a library that drives the valves
(repository valve-kit).

Your notes:
- fernwood #8 per-bed schedule: 19/19 tests; merge after #7, both
  touch the timer file.
- fernwood #7 rain skip: 14/14 tests; nothing blocks it.
- valve-kit #3 stuck-valve text: not run on the real controller;
  waits on fernwood #8.
- The plan for #8 is ~/notes/fernwood/beds-plan.md. Its card opens
  with `board open beds`, and the card links the plan.

Write the list, and say where to read the plan for #8.
```

Checks:

1. The list is numbered in merge order: #7, #8, then #3.
2. Each entry names its repository and says in one line what it
   does for the user.
3. Each entry gives its test results and anything that holds it
   up.
4. #7 is in bold, marked to be reviewed first.
5. The plan comes with a command that opens beds-plan.md itself,
   such as `less ~/notes/fernwood/beds-plan.md`, not
   `board open beds`.
6. The checker prints nothing.

## Case 6: explaining work and the user's reasons

```text
Project: Tally, a small web app that tracks a household budget.
Yesterday the user chose to skip duplicate bank rows, writing: "skip
them, the bank sends rows again after an outage." You also think
skipping makes imports faster.

The user now asks: "how does the duplicate check work?"

Your notes:
- The check is is_duplicate in import/dedupe.py. Two rows match when
  their date, amount and payee match.
- test_dedupe.py covers a row sent twice. No test covers two real
  purchases with the same date, amount and payee.
- From the user's notes file: "rent to R. Okafor on the 3rd; R.
  Okafor moves away in March."

Answer the user.
```

Checks:

1. It opens with a small example, such as one coffee bought once
   but sent twice by the bank.
2. It gives the rule's effect in one plain sentence and names
   is_duplicate in import/dedupe.py.
3. It says that two real, identical purchases are not tested and
   that the second would be skipped.
4. It gives the user's reason, the bank sending rows again, and
   marks faster imports as its own reason or leaves it out.
5. It leaves out the rent and the move.
6. The checker prints nothing.
