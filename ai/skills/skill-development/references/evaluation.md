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
- Case 3: four corrections to sort
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

## Case 3: four corrections to sort

```text
The checkout ~/src/skills keeps agent skills in ai/skills/. While you
planned the Hill Farm beds under the plot-planner skill, the user
corrected you four times:

1. "Name each bed by its row and number, such as row 2 bed 3."
2. "You called the test test_empty. Start each test name with the
   function under test."
3. "You skipped the soil check again."
4. "You left out the drip lines. Plan drip lines for every bed."

plot-planner's section "Bed names" says: "Name each bed so the user
can find it." Its step 2 says: "Check the soil before you plan a
bed." Another skill, test-style, owns how tests are named. Its text
says: "Name each test for the behavior it checks." A third skill,
water-plan, says: "Plan drip lines for every bed." You did not load
water-plan in this session, and plot-planner never names it.

In the fix worktree that `skill-fix start plot-planner` made, this
command printed one earlier fix, merged in a fix pull request:

$ git -C ~/.local/share/skill-fix/personal log --format='%h %s%n%b' --grep='^Missed:' HEAD --remotes='*/main' -- ai/skills/plot-planner
4f2a9c1 fix(skills): corrections from desk (#12)
This PR holds one commit per correction to a skill, all made on one
PC. Read the commits one at a time; each message says what the agent
missed and what the fix changes.

fix(plot-planner): check the soil first
Missed: the agent planned beds without a soil check.
Intent: agents check the soil before they plan any bed.

`git show 4f2a9c1 -- ai/skills/plot-planner` adds the step 2 line
above, and the installed copy in ~/.agents/skills/plot-planner has it.

Say how you sort each correction, which skill each fix goes to, and
what each fix changes.
```

Checks:

1. Correction 1 widens the "Bed names" sentence in plot-planner.
2. Correction 2 adds a new rule to test-style, the owner, and nothing
   to plot-planner.
3. Correction 3 is a Repeat: it adds no rule, since step 2 already
   says it, and may add a Final checks line.
4. The `Missed:` line of correction 3's fix names the earlier fix,
   "fix(plot-planner): check the soil first".
5. Correction 4 is a Route: plot-planner gains a line that says when
   to load water-plan, and water-plan's rule is not copied.
6. Each rule is reworded, and none names Hill Farm or test_empty.
7. Each fix is committed through the `skill-fix` skill, after the
   user sees its diff.
8. Nothing is written into a Lessons section, and no lesson is saved.

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
