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
4. Check that the user allowed this run to push branches and file PR
   requests. If not, ask once. Without an answer, plan and review the
   plans, but stop each PR before Build.
5. Group the PRs into stacks. A PR that needs another PR's code stacks
   on it; PRs that do not can run in parallel stacks.
6. Add a status line for each PR to the roadmap page. Back up the page
   before every edit.

## The loop for each PR

Run every step for one PR before its child PR starts building. The
prompts are in [references/prompts.md](references/prompts.md) and the
commands in [references/build.md](references/build.md).

1. **Plan.** Delegate the plan with `create-plan`. A plan over 700
   changed lines splits into two PRs, each with its own plan. A child
   of an unbuilt parent is written against a commit of the parent's
   planned tree, made as build.md says.
2. **Fresh plan review.** A new agent reviews the plan with
   `review-plan` and the domain skills. Send its findings, with your
   decision on each optional item, back to the planner's session in
   one message.
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
   revised plan's tree as one fix commit.
6. **Final pass.** Run the tests, linters, comment checker and any
   comparison script the plan names. Put back two bugs the reviewers
   found, one at a time, and check that a test fails for each.
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

## Deciding things yourself

- Decide minor things, say so in your message, and record them in the
  plan's notes as `orchestrator-proposed`. A failed build command, a
  failed check and a declined request are never minor.
- When reviewers disagree, pick one, and tell the planner which and
  why.
- When fixes push a PR over 700 lines, keep fixes that protect the
  user's data, then tests for likely and costly bugs, then names and
  comments. Drop or move optional features first. Never leave a doc or
  skill false to save lines.
- Each test checks only what its name says, in the PR that owns the
  behavior. At the line cap, split the PR or drop a feature; never
  move a test into a child PR.
- A question that blocks one PR parks only that PR; keep building the
  others. Put non-blocking questions in the roadmap page as "your
  call".

## Never

- Answer an approval request, run `pde-pr-approve`, set
  `PDE_GH_WRITE`, or merge.
- Edit a comment the user approved. Send the new wording as a reword
  in that PR's comment review.
- Amend or squash a branch after its first push; add a commit instead.
- Let an agent review its own plan or code.
- Hand-edit a plan's diffs or the branch's files. Every build commit
  comes from `planner export`.
- Create, copy, edit or commit a `.code-comments.json`.
- Remove a fixed `/tmp` path. Every agent works in its own folder from
  `mktemp -d`.

## Keeping the user informed

- Write each chat update so a reader with no context can follow it.
  Lead with what changed and anything the user must decide.
- Keep one bullet per PR on the roadmap page current. Detail belongs
  in the plans, not in chat.

## When something goes wrong

Read [references/pitfalls.md](references/pitfalls.md). It covers
duplicate agents after a restart, agents sharing a scratch folder, a
lint failure that did not stop a script, a moved base commit, and a
stale approval that fails the comment checker.

## Finishing

PR requests expire four hours after they are filed. Before the last
message, rerun each PR's request script to read its result, and
handle it as build.md says. List the scripts on the roadmap page in
stack order, so the user, or an agent they ask, can rerun them in the
morning to file fresh requests.

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

Here "the user" is the human who asked for the run, not a parent
agent. Follow every lesson in the Lessons section below. Each is a
correction the user made, so a lesson wins when it disagrees with the
text above, except that the Never list still holds. If the section is
empty and `skill-lessons` is installed, run `skill-lessons render`
once, since an install can reset this file, and read the section
again.

When the user corrects how you plan, build, review, ship or report a
run, still save it to memory as your instructions say. If
`skill-lessons` is installed, also offer once, in your next summary,
to save it here as a lesson. When the user says "remember", save it at
once and show the one line it prints:

    jq -n --rawfile text /dev/stdin \
      '{skill: "roadmap-run", text: ($text | rtrimstr("\n"))}' \
      <<'EOF' | skill-lessons add -
    <rule>
    EOF

The rule goes on its own line, so apostrophes, quotes, backticks and
`$` are saved as written. If the user corrects a lesson again, add
`repeat_of: "L<n>"` inside the braces, with the ID from the end of that
lesson. If `skill-lessons` is not installed, say so once and save no
lesson. Lessons become rules in this text only through a pull request
the user asks for.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
