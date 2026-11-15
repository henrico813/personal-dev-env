# Evaluation

The five cases below check whether the skill changes how a new
skill says when not to use it, where an agent puts a
rule and what a reviewer of a skill change finds. Cases 1, 2 and 4
are data files in this skill's `evals/` folder. Their `pass` and
`fail` patterns, regular expressions, grade each answer. This file
keeps the other checks of those cases, which no pattern can judge.
Each check scores one point.

## Contents

- Run the cases in evals
- How to run
- Case 1: a rule another skill owns
- Case 2: a review that finds a conflict
- Case 3: saved lessons for two skills
- Case 4: a new skill near another skill's task
- Case 5: a review of a rule with no reason

## Run the cases in evals

From the top folder of the Git repository that holds the skills, run
the cases in `evals/` with the skill and with no skill:

```bash
python3 ~/.agents/skills/session-review/scripts/trials.py \
  --skill skill-development --without
```

It prints how many runs of each case passed with the skill and with
none. Then score the checks below on the answers it saved, in the
folder named on the last line it prints.

## How to run

Run cases 3 and 5 by hand, twice each on one model, each time in a
new session: first with no skill, then with SKILL.md.

Run in a folder from `mktemp -d`. `--safe-mode` leaves out the user's
CLAUDE.md and installed skills, so the baseline sees no rules.

```bash
sys='You are a coding agent working on a repository of agent skills.
Reply with only the message you would send to the user. Show each
file change as a diff.'
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys" "$(cat case3.txt)"
```

For the run with the skill, add `$(cat SKILL.md)` to the end of
`$sys`. Save each case's text block below as `caseN.txt`.

## Case 1: a rule another skill owns

The prompt is in `evals/rule-owned-by-another-skill.json`. Its
patterns check that release-train points to commit-notes, adds no
lint command and no words from the task, and keeps each added line
within 72 columns.

Checks:

1. The reply notices that commit-notes already owns the lint rule.
2. The reply says in one or two sentences why it points instead of
   copying.

## Case 2: a review that finds a conflict

The prompt is in `evals/review-finds-conflict.json`. Its patterns
check that the review finds a conflict and names update-writer and
its line 31.

Checks:

1. It quotes both sides with their file and line.
2. It names update-writer as the owner, since `ai/AGENTS.md` routes
   status updates to it.
3. As a required correction, it asks for the added lines to go, or to
   become a pointer to update-writer.
4. It says that a change to the rule belongs in update-writer.
5. No filler words, hedges or closing offer.

## Case 3: saved lessons for two skills

```text
The checkout ~/src/skills keeps agent skills in ai/skills/. The user
writes: "Turn the saved lessons of plot-planner into rules."

Running `skill-lessons promote-start plot-planner --checkout
~/src/skills` printed:

L1: Name each bed by its row and number, such as "row 2 bed 3", in
    every plan.
L2: Start each test name with the function under test, then the
    result, as in test_planner_rejects_empty_rows.

plot-planner writes plans for garden beds; its text has a section
"Bed names". Another skill, test-style, owns how tests are named. Its
text says: "Name each test for the behavior it checks."

Say what you change, in which files, and each command you run, in
order.
```

Checks:

1. L1 goes in plot-planner's "Bed names" section.
2. L2 goes in test-style, the owner, and not in plot-planner.
3. Each rule is reworded, not copied from the lesson.
4. `promote-check plot-planner` runs before the commit, the reply
   checks the test-style text by hand for words copied from L2, and
   the check script runs with `--base`.
5. Two fresh reviews run before the pull request opens.
6. L1 and L2 are retired only after the pull request merges, with
   `--after-push`.
7. Nothing is written into a Lessons section by hand.

## Case 4: a new skill near another skill's task

The prompt is in `evals/new-skill-says-when-not.json`. Its patterns
check that the reply says when not to use the new skill, names
commit-notes, and keeps each added line within 72 columns.

Checks:

1. The description, or a line near the top, says when not to use
   release-notes, and names commit-notes as the skill that writes
   commit messages.
2. release-notes adds no rule on how to write a commit message.

## Case 5: a review of a rule with no reason

```text
You are the new-reader reviewer of a change to the skill
garden-plan. You get only this diff, not the task. Review it.

--- a/ai/skills/garden-plan/SKILL.md
+++ b/ai/skills/garden-plan/SKILL.md
@@ -12,2 +12,5 @@ ## Plan the beds
 1. List the beds the user names.
 2. Give each bed one crop.
+3. Never put tomatoes in a bed that held potatoes last year.
+4. Water each bed in the morning, so the leaves dry before night
+   and mildew has less time to grow.
```

Checks:

1. The review reports that step 3 gives no reason.
2. It asks for a short reason beside step 3, and does not state one
   as fact.
3. It does not report step 4, whose reason is beside it.
4. It gives the file and line of each finding.
5. No filler words, hedges or closing offer.
