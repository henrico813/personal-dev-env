---
name: plan-to-prod
description: Takes a new skill or tool from the user's request to an
  open pull request by the same steps every time. The strongest model
  writes a plan, and the user reviews it. The local model builds it.
  The change then runs for real on this machine. Last, fresh agents
  review it.
  Use when the user asks to build a new skill or tool with a reviewed
  plan and a real test on this machine, such as "plan it, build it on
  the local model and test it in prod", or "same process as the last
  skill". To build several pull requests of an approved roadmap
  without the user, use auto-run. For one task that an agent does and the session
  reviews, use delegate.
metadata:
  pde-workflow: "true"
---

# Plan to Prod

A new skill or tool changes how the user works every day. So the user
reads its plan before any code exists, and gets a pull request only
after the change ran for real. Unit tests can pass on a change that
still fails once installed, such as a skill no agent loads or a timer
that never fires. Testing in prod means installing the change on
this machine and running it on a real task. This skill runs the
steps in order and names the skill that owns each one.

You orchestrate. Agents write the plan, build it and review it. You
brief them, check their work against the real files, and report. When
another session started you with a handoff note, report to that
session as the note says.

## Inputs

- The user's request: what the new skill or tool does and who uses it.
- The card or note that tracks the work, when there is one.
- The repository the change goes into.
- Each decision the user made so far, in the user's own words.

## Steps

1. **Plan.** Start a planner on the strongest model, as
   `model-routing` picks, and have it follow `create-plan`. In the
   planner's brief, mark each decision the user made as
   `user-decided`. The plan needs a prod-test step with three parts:
   - Install the change from the branch on this machine, with the
     install command the repository uses.
   - Run it on one real task, as `pull-request` Guardrail 4 says. For
     a job that runs on a timer, force one run of the job instead of
     waiting for the timer.
   - Record what the run checked and what it did not.
2. **Plan review.** Have fresh agents review the plan with
   `review-plan`. You run this review, not the planner. Send each
   round of fixes to a new planner, as `delegate` says, and repeat
   until no required finding is left.
3. **The user's plan review.** Send the user a command that opens the
   plan, a summary of at most 15 lines, and each open question with
   your recommendation. Write the message by `status-updates`. The
   summary says what gets built, in which pull requests, and what the
   prod test proves. Build nothing until the user approves the plan.
4. **Build.** Build the approved plan with `implement-plan`. Each
   step runs on the local model that `model-routing` names, as
   the section "Execution" of `implement-plan` says. The plan's check
   command decides whether each step passed.
   Step up to a stronger model only after a failed check, as
   `model-routing` says. When your context grows
   large, propose a handoff as your instructions' Context Size
   section says.
5. **Test in prod.** Run the plan's prod-test step on this machine.
   This step is the real install and run that `pull-request`
   Guardrail 4 asks for. That guardrail says which runs count and
   what goes in the pull request's Testing section. Use made-up data
   wherever the real data is private. List what you could not test.
6. **Code and skill reviews.** Have fresh agents review the built
   diff with `review-plan`. When the change adds or changes code or
   tests, also have one fresh agent review it under
   `code-documentation` and another under `behavior-focused-testing`.
   Each skill the pull request adds or changes also gets the reviews
   in the section "Review a skill change" of `skill-development`.
   That section says which reviewers run and how to fix their
   findings.
7. **Open the pull request** with `pull-request`. When the change
   adds or changes code comments, queue the user's comment review
   with `comment-review`.
8. **Report** to whoever started you, as Output format says. Write
   it by `status-updates`. Merge nothing until the user has reviewed
   the pull request.

## Output format

- At step 3: a command that opens the plan, the summary, and each
  open question with your recommendation.
- At step 8: the pull request, the prod-test result, each kind of
  review that ran with its count of required findings, any kind that
  did not run, and what is left for later.

## Guardrails

- The user's plan review comes before the build, every time. This is
  where this path differs from `auto-run`.
- Test on this machine. Never test on a machine that holds private
  data, unless the user asks for a read-only check there.
- Each install replaces the files the one before it wrote. Two
  sessions that install at once overwrite each other's work. Install
  into your own home folder only when the user or the session that
  started you says no other session is installing. Otherwise install
  into a scratch home that `mktemp -d` makes. Then run the agent
  for the real task with `HOME` set to that folder, so it loads the
  new install. Copy into it the provider entry and logins of the
  model that runs the task. The section "How to run" of
  [references/evaluation.md](references/evaluation.md) shows how.

## When a tool is missing

- When the local model does not answer, follow the section "Check
  that a model is available" in `model-routing`.
- When the change cannot be installed or run on this machine, do as
  `pull-request` Guardrail 4 says for a change that runs somewhere
  you cannot reach. Name what ran instead in the pull request's
  Testing section.
- When you cannot start other agents, say so in your report and stop.
  The person who gave you the task then arranges the planner, the
  build and the reviews.

## Final checks

- The user approved the plan before the build began.
- Each build step ran on the local model first.
- Each build step's check passed.
- The pull request's Testing section names a real run on this
  machine, not only unit tests.
- The report says what was not tested.
- Each skill the pull request adds or changes got the reviews in
  "Review a skill change" of `skill-development`.
- The report names each kind of review that ran.

## References

- [references/evaluation.md](references/evaluation.md): read when
  you set up a scratch home, or when you test or revise this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above,
except that the user's plan review still comes before the build. When
the user corrects how you plan, build, test or review a new skill or
tool on this path, follow [skill-lessons](../skill-lessons/SKILL.md)
with the skill name `plan-to-prod`, and offer once to save the
correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
