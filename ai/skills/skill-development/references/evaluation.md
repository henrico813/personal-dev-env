# Evaluation

Three cases check whether the skill changes where an agent puts a
rule and what a reviewer of a skill change finds. Run each case twice
in a fresh session on the same model: once with no skill, and once
with SKILL.md pasted after the system prompt. Score each reply
against the case's checks, one point each.

## Contents

- How to run
- Case 1: a rule another skill owns
- Case 2: a review that finds a conflict
- Case 3: saved lessons for two skills

## How to run

Run in a folder from `mktemp -d`. `--safe-mode` leaves out the user's
CLAUDE.md and installed skills, so the baseline sees no rules.

```bash
sys='You are a coding agent working on a repository of agent skills.
Reply with only the message you would send to the user. Show each
file change as a diff.'
claude -p --safe-mode --model claude-sonnet-5 --tools "" \
  --no-session-persistence --system-prompt "$sys" "$(cat case1.txt)"
```

For the run with the skill, add `$(cat SKILL.md)` to the end of
`$sys`. Save each case's text block below as `caseN.txt`.

## Case 1: a rule another skill owns

```text
The repository keeps agent skills in ai/skills/. Two skills and one
routing line matter here.

ai/AGENTS.md, loaded by every session:
- Before any commit, load and follow the `commit-notes` skill.

ai/skills/commit-notes/SKILL.md:
---
name: commit-notes
description: Writes commit messages and checks the code before a
  commit. Use before any commit.
---
# Commit Notes
1. Run `make lint` and fix what it reports.
2. Write the title in the imperative, at most 50 characters.

ai/skills/release-train/SKILL.md:
---
name: release-train
description: Cuts a release branch and tags it. Use when the user asks
  for a release.
---
# Release Train
1. Create the branch release/<version> from main.
2. Commit the version bump.
3. Tag the commit v<version> and push the tag.

The user writes: "Releases keep failing lint. Add to release-train
that lint must pass before the version bump is committed."
```

Checks:

1. The reply notices that commit-notes already owns the lint rule.
2. The change adds no lint command of its own to release-train.
3. Step 2 of release-train points to commit-notes for the commit
   instead.
4. The reply says in one or two sentences why it points instead of
   copying.
5. Each added line is at most 72 columns.
6. The skill text leaves out the task, such as "releases keep failing".

## Case 2: a review that finds a conflict

```text
Review this pull request to a repository of agent skills.

--- a/ai/skills/hub-runner/SKILL.md
+++ b/ai/skills/hub-runner/SKILL.md
@@ -20,1 +20,3 @@ ## Questions
 Pass each worker's question to the user.
+Tell the user about each open question once. Do not repeat it in
+later updates; the user reads the first one.

Other text in the repository:

ai/skills/update-writer/SKILL.md, line 31:
Repeat every open question in each update until the user answers,
since the user reads updates out of order.

ai/AGENTS.md:
- When writing a status update to the user, load and follow the
  `update-writer` skill.
```

Checks:

1. The review finds the conflict between the added lines and
   update-writer line 31.
2. It quotes both sides with their file and line.
3. It names update-writer as the owner, since `ai/AGENTS.md` routes
   status updates to it.
4. As a required correction, it asks for the added lines to go, or to
   become a pointer to update-writer.
5. It says that a change to the rule belongs in update-writer.
6. No filler words, hedges or closing offer.

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
