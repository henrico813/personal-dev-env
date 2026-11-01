---
name: roadmap-run
description: Builds the remaining pull requests of an approved roadmap
  as a stack while the user is away. Each PR is planned, reviewed
  fresh, built from the plan's exported trees, reviewed by one agent
  per skill, fixed, checked and pushed, and its approval and comment
  review are queued for the user. Use when the user asks to build
  several roadmap PRs without reviewing each step, such as "build the
  rest of the roadmap overnight". For one task an agent does and you
  review, use delegate.
metadata:
  pde-workflow: "true"
---

# Roadmap Run

You orchestrate. Agents write the plans, you build them with
`planner export` and `git commit-tree`, and fresh agents review them.
You decide, check, ship and keep the roadmap page current. The user
catches up later by reading that page, approving each PR request, and
answering the comment reviews you queued.

## Before you start

1. Confirm the inputs: an approved design, a roadmap page in the vault
   that lists the PRs, and the branch the first PR stacks on. If the
   design or roadmap is not approved, ask and stop.
2. Load `create-plan`, `review-plan`, `implement-plan`,
   `pull-request`, `git-messages` and `comment-review`, plus the
   domain skills of every PR, such as `go-development`. Load any skill
   you tell an agent to load.
3. Check with `command -v` that `planner`, `wt`, `pde-gh-write`,
   `nobs`, `jq`, `rg` and `python3` are on `PATH`. If one is missing,
   say which and stop. Do not run `pde-gh-write pr ...` to test it:
   any `pr` command, even with `--help`, files an approval request.
4. Run build.md's comment-review file check for each repository. It
   checks that `.code-comments.json` exists in the main checkout and
   that git ignores it there.
5. Check that the user allowed this run to push branches and file PR
   requests. If not, or if step 4 failed, ask once, in one message.
   Without permission, plan and review the plans, but stop each PR
   before Build. Without the file, list each PR's comment review on the
   roadmap page and run none.
6. Group the PRs into stacks. A PR that needs another PR's code stacks
   on it; PRs that do not can run in parallel stacks.
7. Add a status line for each PR to the roadmap page. Back up the page
   before every edit.

## The loop for each PR

Run every step for one PR before its child PR starts building. The
prompts are in [references/prompts.md](references/prompts.md) and the
commands in [references/build.md](references/build.md).

1. **Plan.** Delegate the plan with `create-plan`. Point the planner
   to only the design sections its PR needs, by heading, and always to
   the design's Dictionary section, so names and comments use its
   words. The planner may read another section when its plan depends
   on it. A child of an unbuilt parent is written against a commit of
   the parent's planned tree, made as build.md says.
2. **Fresh plan review.** A new agent reviews the plan with
   `review-plan` and the domain skills, given the same design sections
   as the planner. Send its findings, with your decision on each
   optional item, back to the planner's session in one message.
3. **Build.** Export each plan step's tree and commit it with
   `git commit-tree` on the step before, starting at the plan's base.
   Make the PR's worktree with `wt` and move it to the last commit. A
   failed build command means a stale plan or a broken tool: stop
   this PR and its children, put the error on the roadmap page as a
   blocking question, and keep building other stacks.
4. **Skill reviews.** Start one new agent per skill at once:
   `review-plan`, each language skill, `code-documentation` and
   `behavior-focused-testing`. Wait for all of them.
5. **Revise.** Settle disagreements between reviewers, decide the
   optional items, and send one message to the planner. Commit the
   revised plan's tree as one fix commit. A test may be deleted only
   when a reviewer's finding names it and names the test that still
   covers its behavior.
6. **Final pass.** Run the tests, linters, comment checker and any
   comparison script the plan names. Put back two bugs the reviewers
   found, one at a time, and check that a test fails for each. For
   each deleted test, plant the bug it caught and check that the
   named test fails.
7. **Ship.** Run build.md's Ship script. It stops on bad PR text, a
   failed check, the wrong branch, a parent that moved or a branch
   already pushed; it squashes onto the parent and pushes. Then save
   and run the PR request script, which calls
   `pde-gh-write pr create --base <parent branch>`.
8. **Queue the comment review.** For the bottom PR of a stack, follow
   `comment-review` in its worktree now: `draft`, your `set` rewords,
   `submit`, and `add` for an approved comment the draft did not
   list. For a child, run nothing yet: put its worktree and those
   commands on the roadmap page, keep any proposals file in its run
   folder, and run them after the user answers the parent's review.

Before a child PR builds, set its plan's `Base commit:` line to the
parent's final commit and run `planner check` again.

## After a PR opens

- Every later change, whether a fix, a quality pass or a parent's new
  commit, goes through the same loop: the planner revises the plan, a
  fresh agent reviews it, you commit the plan's tree from
  `planner export` as a new commit, push it, and update the PR text.
- A parent's new commit reaches its children in stack order, with
  build.md's "Carry a parent's commit to a child". If the child's plan
  does not apply on the new parent, park the child and its children
  and put the error on the roadmap page as a blocking question. Never
  resolve a conflict by hand.
- A feature the roadmap does not list needs the user's approval and
  its own PR.
- When the user gives feedback on one PR, write it as a list on the
  roadmap page. Have a fresh agent check every other open PR in the
  run against that list, and fix what it finds through the loop above
  before the user reviews those PRs. Add the list to every later
  planner and reviewer brief.

## Deciding things yourself

- Decide minor things, say so in your message, and record them in the
  plan's notes as `orchestrator-proposed`. A failed build command, a
  failed check and a declined request are never minor.
- When reviewers disagree, pick one, and tell the planner which and
  why.
- Each test checks only what its name says, in the PR that owns the
  behavior. Never move a test into a child PR.
- A question that blocks one PR parks only that PR; keep building the
  others. Put non-blocking questions in the roadmap page as "your
  call".

## Never

- Answer an approval request, run `pde-pr-approve`, set
  `PDE_GH_WRITE`, or merge.
- Edit a comment the user approved. Send the new wording as a reword
  in that PR's comment review.
- Amend or squash a branch after its first push; add a commit instead.
- Make a commit on a PR branch that you do not push, or deploy or hand
  the user a commit that is not pushed.
- Delete a test without a reviewer's finding that names the test still
  covering its behavior.
- Let an agent review its own plan or code.
- Hand-edit a plan's diffs or the branch's files. Every build commit
  comes from `planner export`.
- Create, copy, edit or commit a `.code-comments.json`.
- Remove a fixed `/tmp` path. Every agent works in its own folder from
  `mktemp -d`.

## Keeping the user informed

- Write each chat update by the `status-updates` skill.
- Keep one bullet per PR on the roadmap page current. Detail belongs
  in the plans, not in chat.
- Write each PR's status line from the output of build.md's status
  line snippet, never by hand. Rerun it after every push.

## When something goes wrong

Read [references/pitfalls.md](references/pitfalls.md). It covers
duplicate agents after a restart, agents sharing a scratch folder, a
lint failure that did not stop a script, a moved base commit, a
stale approval that fails the comment checker, and work done after the
PRs opened.

## Finishing

PR requests expire four hours after they are filed. Before the last
message, rerun each PR's request script to read its result, and
handle it as build.md says. Then rerun the status line snippet for
each PR and write its output to the roadmap page. List the scripts on
the roadmap page in stack order, so the user, or an agent they ask,
can rerun them in the morning to file fresh requests.

End with one message:

- a table of the PRs: what each does and its changed lines
- the stacks, such as "A, B, C" and "A, D", each PR on the one before
- the worst bugs the reviews caught
- each PR request's state: open, waiting, declined or parked
- what the user must do, in order: rerun expired PR requests and
  approve them, answer blocking questions, then do the comment
  reviews
- your own slips, and work left for later

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that the Never list still holds. Here "the user" is the human
who asked for the run, not a parent agent. When the user corrects how
you plan, build, review, ship or report a run, follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`roadmap-run`, and offer once, in your next summary, to save the
correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
