---
name: roadmap-run
description: Builds the remaining pull requests of an approved roadmap
  as a stack while the user is away. Each PR is planned, reviewed
  fresh, built with Vibe, reviewed by one agent per skill, fixed,
  checked and pushed, and its approval and comment review are queued
  for the user. Use when the user asks to build several roadmap PRs
  without reviewing each step, such as "build the rest of the roadmap
  overnight". For one task an agent does and you review, use delegate.
metadata:
  pde-workflow: "true"
---

# Roadmap Run

You orchestrate. Agents write the plans, Vibe builds them, and fresh
agents review them. You decide, check, ship and keep the roadmap page
current. The user catches up later by reading that page, approving
each PR request, and answering the comment reviews you queued.

## Before you start

1. Confirm the inputs: an approved design, a roadmap page in the vault
   that lists the PRs, and the branch the first PR stacks on. If the
   design or roadmap is not approved, ask and stop.
2. Load `create-plan`, `review-plan`, `implement-plan`,
   `pull-request`, `git-messages` and `comment-review`, plus the
   domain skills of every PR, such as `go-development`. Load any skill
   you tell an agent to load.
3. Check with `command -v` that `planner`, `vibe`, `pde-gh-write`,
   `nobs` and `jq` are on `PATH`. If one is missing, say which and
   stop. Do not run `pde-gh-write pr ...` to test it: any `pr`
   command, even with `--help`, files an approval request.
4. Group the PRs into stacks. A PR that needs another PR's code stacks
   on it; PRs that do not can run in parallel stacks.
5. Add a status line for each PR to the roadmap page. Back up the page
   before every edit.

## The loop for each PR

Run every step for one PR before its child PR starts building. The
prompts are in [references/prompts.md](references/prompts.md) and the
commands in [references/build.md](references/build.md).

1. **Plan.** Delegate the plan with `create-plan`. A plan over 700
   changed lines splits into two PRs, each with its own plan. A child
   of an unbuilt parent is written against the parent's planned tree.
2. **Fresh plan review.** A new agent reviews the plan with
   `review-plan` and the domain skills. Send its findings, with your
   decision on each optional item, back to the planner's session in
   one message.
3. **Build.** Export each plan step's tree, turn it into a patch, and
   have Vibe apply it with `git apply`. After each step, the branch
   tree must equal the exported tree. Stop on any mismatch.
4. **Skill reviews.** Start one new agent per skill at once:
   `review-plan`, each language skill, `code-documentation` and
   `behavior-focused-testing`. Wait for all of them.
5. **Revise.** Settle disagreements between reviewers, decide the
   optional items, and send one message to the planner. Vibe applies
   the revised plan as one fix commit; the tree must match again.
6. **Final pass.** Run the tests, linters, comment checker and any
   comparison script the plan names. Put back two bugs the reviewers
   found, one at a time, and check that a test fails for each.
7. **Ship.** Squash the branch onto its parent, lint the message, push
   with `--force-with-lease`, and file the PR request with
   `pde-gh-write pr create --base <parent branch>`.
8. **Queue the comment review.** In a notes file outside the
   repository, write the PR's worktree and the `comment-review` steps
   to run there: `pde-comment-review draft`, each `set` reword you
   decided, and `submit`. Link it from the roadmap page. Run them
   only after the user answers the parent's review.

Before a child PR builds, set its plan's `Base commit:` line to the
parent's final commit and run `planner check` again.

## Deciding things yourself

- Decide minor things, say so in your message, and record them in the
  plan's notes as `orchestrator-proposed`.
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
- Hand-edit a plan's diffs, or give Vibe diffs in its prompt instead
  of a patch file.
- Edit or commit a `.code-comments.json`.
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

End with one message:

- a table of the PRs: what each does and its changed lines
- the stacks, such as "A, B, C" and "A, D", each PR on the one before
- the worst bugs the reviews caught
- what the user must do, in order: approve PR requests, answer
  blocking questions, then do the comment reviews
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
