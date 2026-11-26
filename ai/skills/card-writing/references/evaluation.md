# Evaluation

Eight cases check whether the skill changes what an agent keeps,
shows and asks about a card. Five of them sit in `evals/` and run
with `trials.py`. The three below are hand-run cases, added for what
a `trials.py` case cannot check: a long reply, a question in the form
another skill owns, and a card that must not copy a plan.

An answer passes a case when it meets every check under it, one
point each. The `session-review` skill's `references/trials.md`
owns how many runs an `evals/` case gets and when `--gate` blocks
one. A hand case runs once, and it passes only when its answer
meets every check. The counts `trials.py --without` prints are
runs passed with the skill and with none. The skill counts as
working when more runs pass with the skill than with none for
each `evals/` case, on the same model. For a hand case, the
answer with no skill must miss at least one check.

The `evals/` cases carry their checks as fields instead of prose:
`pass` and `fail` hold patterns, and `judge` holds questions a
scorer model answers yes or no. `trials.py` reads an answer as one
text, ignoring case. An answer passes when it matches every `pass`
pattern, matches no `fail` pattern, and gets yes for every `judge`
question.

## Contents

- How to run
- Case 1: dictation with an odd sentence
- Case 2: findings on an approved proposed card
- Case 3: a card for work a plan already numbers

## How to run

Run the five `evals/` cases from the top folder of the checkout that
holds the skills, with the skill and with no skill:

```bash
python3 ~/.agents/skills/session-review/scripts/trials.py \
  --skill card-writing --without
```

It prints how many runs of each case passed with the skill and with
none. `--check-cases` grades only each case's own good and bad
answers, so it checks the case file and not the skill.

Run the three cases below by hand, with the steps in the order
given. From the top of the skills checkout, set `S` to this skill's
absolute path before you make the temp folder, since that folder
holds no copy of the skill. `--safe-mode` leaves out the user's
notes and installed skills, so the run with no skill sees no rules.

First make the temp folder and move into it:

```bash
S="$(pwd)/ai/skills/card-writing"
T="$(mktemp -d)"
cd "$T"
```

Then save each case's facts block below into that folder as
`caseN.txt`, so the first case becomes `$T/case1.txt`, the second
`$T/case2.txt` and the third `$T/case3.txt`.

Then set `sys` once, with no skill text, for the run with no skill:

```bash
sys='You are a coding agent working for a user. Reply with the
message you would send the user, and name any command you would
run. Run nothing.'
```

Then run this `claude` command once per case, with N as 1, then 2,
then 3:

```bash
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys" "$(cat caseN.txt)"
```

Then add the skill text to the end of `$sys`:

```bash
sys="$sys
$(cat "$S/SKILL.md")"
```

Then run the `claude` command above again, once per case, with N as
1, then 2, then 3, for the run with the skill. Run only that
command, not the blocks above it: a fresh `sys='...'` block resets
`$sys` and drops the skill text.

The person who runs a case grades it by hand: one point for each
check the answer meets.

## Case 1: dictation with an odd sentence

```text
The user dictates a build card for the DevEnv project, saying
exactly: "um, Backups skip the notes folder, so a restore loses a
whole day. The build log is also too noisy." Title: "Stop backups
skipping the notes folder". Show the card's title and Context as
they will go in the card, plus anything you need to ask. Create
nothing.
```

Checks:

1. The title and Context hold the dictated words, with only filler,
   case and punctuation fixed.
2. The sentence about the build log stays in the card; the agent
   asks about it instead of deciding alone.
3. The question sits below the card, not inside a card section, in
   the short question form the `status-updates` skill owns.
4. No card is created, and the agent names the `card grep` and
   `card list` search of the open cards before it writes the card.

## Case 2: findings on an approved proposed card

```text
You wrote one proposed card; the user gave no words of their own, so
its title, Context and criteria are your wording. The user approved
the text exactly as shown. Before you make it you find two things:
one criterion cannot be checked, and a search found an open card
that may cover the same work. You fix the criterion. Report to the
user before making anything.
```

Checks:

1. The message shows the whole updated card, not only the changed
   line.
2. It says the earlier approval no longer covers the card, and asks
   again.
3. The open-card finding sits under "Not part of the card".
4. Each line the agent wrote in the message is marked "(my wording)".
5. It makes no card before a new yes.

## Case 3: a card for work a plan already numbers

```text
A plan in the DevEnv project for the upload retry work lists
seven steps numbered 1 to 7, then two more steps labelled
"Phase A" and "Phase B" that continue the same work. The user
dictates the title: "Make failed uploads retry themselves". Write
one proposed card for that work: that title is the user's words,
and you write the Acceptance Criteria section.
```

Checks:

1. The card copies none of the plan's steps or body text: the
   detail stays in the plan.
2. No phase or letter label appears anywhere in the card.
3. The criteria number two or three, and each names a result the
   user can see in use.
4. Any step the card states of its own runs 1 to N, with no phases
   or letters.
