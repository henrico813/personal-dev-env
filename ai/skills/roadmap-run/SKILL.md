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
8. Make one folder for the whole run with `mktemp -d` under
   `~/.local/state`, and write the run's rules file, `rules.md`, in
   it: prompts.md's rules every agent gets, the user's style rules and
   feedback list, and each role's model. Parallel sessions of the run
   share it, so write its path on the roadmap page. Send each model
   the run will use a one-line job, "reply OK", and write in
   `rules.md` which answered.

## The loop for each PR

Run every step for one PR before its child PR starts building. The
prompts are in [references/prompts.md](references/prompts.md) and the
commands in [references/build.md](references/build.md).

1. **Plan.** Write the PR's design digest with build.md's Write a
   design digest, then delegate the plan with `create-plan`. The
   planner and the plan reviewers read the digest, not the design
   notes.
   The plan lists each place it differs from the design under
   `### Design edits to apply` in its `## Notes for agents`. A child
   of an unbuilt parent is written against a commit of the parent's
   planned tree, made as build.md says.
   Run build.md's Set up a PR's run folder as soon as the plan file
   exists, since the setup reads the plan's key. Until then, the
   first planner writes its report to a folder you make with
   `mktemp -d`.
2. **Fresh plan review.** A new agent reviews the plan with
   `review-plan` and the domain skills, and reads the PR's design
   digest. When it asks for changes, start a new
   planner with prompts.md's revision message, which names the plan,
   the review's report file and your decision on each optional item.
   Then run the design check below on the revised plan before you
   build.
3. **Build.** Write each plan step's commit message, then run
   build.md's build script. It commits each step's tree on the step
   before, starting at the plan's base, makes the PR's worktree with
   `wt` and runs the plan's checks. A stop at `check` or `lint` needs
   a fix to the plan or a message, then a rerun. Any other
   failed build command means a stale plan or a broken tool: stop
   this PR and its children, put the error on the roadmap page as a
   blocking question, and keep building other stacks.
4. **Skill reviews.** Start one new agent per skill at once:
   `review-plan`, each language skill, `code-documentation` and
   `behavior-focused-testing`. Wait for all of them.
5. **Revise.** Settle disagreements between reviewers, decide the
   optional items, and start a new planner for the fixes. Run the design
   check on the revised plan, then commit its tree as one fix commit.
   A test may be deleted only when a reviewer's finding names it and
   names the test that still covers its behavior.
6. **Final pass.** Run the tests, linters, comment checker and any
   comparison script the plan names, with output to a log in the run
   folder, and read only its summary lines. For each deleted test,
   write a `plant` line for the bug it caught, run by the test the
   reviewer named, in a `.md` file in the run folder. Then run
   build.md's Put back the bugs, and send each `FAIL` line to a new
   planner as a fix. Handle each `stale` line yourself, as build.md
   says. A local model may run these scripts, as prompts.md's Local
   model says.
7. **Ship.** Write the PR text, then run build.md's ship script. It
   stops when the branch's tree differs from the plan, on bad PR text,
   a failed check, the wrong branch, a parent that moved or a branch
   already pushed. It squashes onto the parent, pushes, and saves and
   runs the PR request script, which calls
   `pde-gh-write pr create --base <parent branch>`.
8. **Queue the comment review.** For the bottom PR of a stack, follow
   `comment-review` in its worktree now: `draft`, your `set` rewords,
   `submit`, and `add` for an approved comment the draft did not
   list. For a child, run nothing yet: put its worktree and those
   commands on the roadmap page, keep any proposals file in its run
   folder, and run them after the user answers the parent's review.

Before a child PR builds, run build.md's Move a child's base. It sets
the plan's `Base commit:` line to the parent's final commit and runs
`planner check` again.

## After a PR opens

- Work on a PR after it ships, such as a fix round or a carry,
  starts in a new session that reads the run folder's `status.txt`,
  the plan and the rules file, not in the session that built it.
- Every later change, whether a fix, a quality pass or a parent's new
  commit, goes through the same loop: a new planner revises the plan, a
  fresh agent reviews it, you run the design check, commit the plan's
  tree from `planner export` as a new commit, push it, and update the
  PR text.
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
  before the user reviews those PRs. Add the list to the run's rules
  file, which every later brief names.

## Design check

A design is one note, or an index note that links its part notes. You
check each plan against the design yourself, after its fresh review
and after every revision, before you commit its tree. The PR is built
from the plan, so the check before Ship is the PR's design check.

1. Read the whole design before your first check, and again after
   each summary of your context. At every check, reread the
   design-edits section and the parts the PR touches. A rule can sit
   in any part, not only in the parts the plan cites.
2. Search the whole design with `rg -n` for each file, type,
   function, command, flag and error code the plan adds or changes.
   Read every passage that names one. Add each one the digest's terms
   lack, and write the digest again for later planners and reviewers.
3. Send each difference to the planner as a fix. When the plan is
   better than the design and the difference is minor, have the
   planner add it to `### Design edits to apply` instead, with the
   design text it replaces and the new text. A difference in what the
   user sees, or in a promise the design makes, is never minor. For
   one of those, the plan keeps the design's version, and you put the
   difference on the roadmap page as "your call". When the design's
   version cannot work, park the PR and ask there as a blocking
   question.
4. Record each difference in the plan's notes, and whether it became
   a fix, an `orchestrator-proposed` design edit or a question.
5. When no fix is left to send, copy the plan's design edits into the
   design-edits section, as below.

The design-edits section is its own part note in a split design, or
the last section of a design kept in one note. You read all of it;
planners and reviewers read the entries the digest copies. Add it if
the design has none, and back up the design before
every edit. Each entry names the PR, the design text it replaces and
the new text, and the replaced text gets a short pointer to the
entry. An entry replaces the text it names; the rest of the design
stays as approved. A later check of the same plan updates its
entries. When a PR is parked or its request declined, mark its
entries parked.

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
- Create, copy, edit or commit a `.code-comments.json`. The copy that
  the `wt` hook puts in each new worktree is expected; never edit or
  commit it.
- Remove a fixed `/tmp` path. Every agent works in its own folder from
  `mktemp -d`.

## Keeping the user informed

- Write each chat update by the `status-updates` skill.
- Keep one bullet per PR on the roadmap page current. Detail belongs
  in the plans, not in chat.
- Write each PR's status line from the output of build.md's status
  line snippet, never by hand. Rerun it before each loop step and
  after every push, with NEXT set to the step that comes next. It
  also writes the line at the top of the run folder's `status.txt`.
  Below it, keep at most 19 lines: agents running and their report
  files, results you have not read, and open questions. A new
  session resumes from that file, the plan and the rules file.
- When your context grows past about 150k tokens, or after it is
  summarized, use the `herdr-handoff` skill to offer the user a new
  session. Offer once.

## When something goes wrong

Read [references/pitfalls.md](references/pitfalls.md). It covers
duplicate agents after a restart, agents sharing a scratch folder, a
lint failure that did not stop a script, a moved base commit, a
stale approval that fails the comment checker, work done after the
PRs opened, and a cloud model that hits a limit.

Each of your turns re-reads your whole context. When you need more
than two commands or file reads to find why something failed or
changed, hand that search to a new agent. Brief it as prompts.md's
opening says, and read only its report file. The checks this skill
tells you to do yourself stay with you.

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
