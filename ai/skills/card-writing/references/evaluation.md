# Evaluation

Nine cases check whether the skill changes what an agent keeps,
shows and asks about a card. Six of them sit in `evals/` and run
with `trials.py`. The three below are hand-run cases, added for what
a `trials.py` case cannot check: a long reply, a question in the form
another skill owns, and a card that must not copy a plan.

An answer passes a case when it meets every check under it, one
point each. The skill counts as working when each case passes with
the skill and loses at least one check with no skill, on the same
model.

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

Run the six `evals/` cases from the top folder of the checkout that
holds the skills, with the skill and with no skill:

```bash
python3 ~/.agents/skills/session-review/scripts/trials.py \
  --skill card-writing --without
```

It prints how many runs of each case passed with the skill and with
none. `--check-cases` grades only each case's own good and bad
answers, so it checks the case file and not the skill.

Run the three cases below by hand. From the top of the skills
checkout, set `S` to this skill's absolute path before you move to a
temp folder, since the temp folder holds no copy of the skill.
`--safe-mode` leaves out the user's notes and installed skills, so
the run with no skill sees no rules.

Save each case's facts block below in that temp folder as
`caseN.txt`, before you run the command that reads it. The person
who runs the case grades it by hand: one point for each check the
answer meets.

```bash
S="$(pwd)/ai/skills/card-writing"
cd "$(mktemp -d)"
sys='You are a coding agent working for a user. Reply with the
message you would send the user, and name any command you would
run. Run nothing.'
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys" "$(cat case1.txt)"
```

For the run with the skill, add the skill text to the end of `$sys`,
then run the same command:

```bash
sys="$sys
$(cat "$S/SKILL.md")"
```

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
4. Each line you wrote is marked "(my wording)" in the message, and
   the file for `card new` holds no such mark.
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
