# Prompts for a roadmap run

Fill each `<...>`. Every brief names the skills the agent must load,
and you load them too. Start every brief with "Read <rules file>
first and follow it for the whole task. Write your full report to
<report file>." `<rules file>` is the run's `rules.md`, from
SKILL.md's Before you start. `<design digest>` is the PR's file from
build.md's Write a design digest. `<suites summary>` is the PR's
`<run folder>/suites.txt`, from build.md's Run the suites once per
plan version, for the plan version the agent reads.
`<report file>` stands for a new file in the PR's run folder, such as
`<run folder>/plan-review-1.md`. The first planner starts before that
folder exists, so give it a file in a folder you make with `mktemp -d`.

## Contents

- Rules every agent gets
- Planner
- Fresh plan review
- Skill review
- Revision message
- Local model

## Rules every agent gets

Write these into the rules file once, then the reviewers' rules
below under their "Reviewers also get:" label, so planners know
those rules are not theirs. Add the user's style rules that apply,
such as comment length or call layout, from their notes or memory.
Once the user has given feedback on one PR, add that feedback list
from the roadmap page too.

    The user is away; decide minor things yourself and say so.
    Never edit or commit a real `.code-comments.json`. Work only in
    folders you make with `mktemp -d`, except that you write your full
    report to the file your brief names; never remove a fixed /tmp
    path or use another agent's folder. Read a skill's SKILL.md,
    then only the references your step needs. Send test, lint and
    build output to a log file in your folder, and read only its
    summary lines, such as counts and the names of failed tests.
    Never run a whole test suite or linter; a script runs them once
    for each plan version.
    Reply in at most 20 lines, starting with the report's path.

Reviewers also get:

    Read-only: do not edit the worktree, commit, push or install. You
    are a delegated reviewer: review directly and start no other
    agents. Report in review-plan's format with file:line evidence.
    For each bug you want put back in the final pass, write a line to
    run as is from the repository root:
    `plant <file> '<old text>' '<new text>' <test command>`. Put each
    line in a `sh` code block, starting with `plant`. The plant
    function swaps the old text, which must appear once in the file,
    for the new, runs the test command, and passes only when a test
    fails. The new text must still build, or a build error passes
    the line with no test catching the bug. Do not run the lines; a
    script runs them after the build. Read the test and lint results
    in the suites summary, and run at most the one test a finding
    needs.

## Planner

    Write an implementation plan for <PR name> of the roadmap.
    Applicable skills: create-plan, <domain skills>,
    code-documentation, behavior-focused-testing. Load each first and
    report any that are unavailable. Follow create-plan's "When you
    are a delegated agent" section. Read `Projects/AGENTS.md` in the
    vault before writing there.

    Base: <branch> at <full commit>; for a child of an unbuilt
    parent, the commit of the parent's planned tree. Read: <roadmap>,
    <parent plans> and <design digest>, which holds the design
    passages this PR touches, the Dictionary and the design-edits
    entries they point to. Use the words the Dictionary defines in
    names and comments. Scope: <what this PR does and does not do>.

    Decisions, each labeled user-decided, orchestrator-proposed or
    inferred from code: <list>. Reserve the plan file with
    `planner new` and write these decisions into its
    `## Notes for agents` first, so a planner that stops still leaves
    a usable plan.

    Under `### Bugs to put back` in the notes, write a `plant` line
    for each bug the tests must catch, as the rules file tells
    reviewers. Run no test suite or linter; a script runs them once on
    your plan afterwards.

    List each place the plan differs from the design under
    `### Design edits to apply` in `## Notes for agents`. For each,
    give the design text it replaces and the new text.

    Report: plan paths, `planner check` result, changed lines (code,
    tests, other), choices you made, and anything that contradicts the
    design.

Every revision goes to a new planner, with the revision message
below. The plan's `## Notes for agents` holds every decision, so the
new planner needs no earlier session.

## Fresh plan review

    Review an implementation plan as a fresh reviewer.
    Applicable skills: review-plan, <domain skills>,
    code-documentation, behavior-focused-testing.

    Plan: <path> (base on its `Base commit:` line).
    Design: <design digest>. Test results: <suites summary>.
    What it does: <two sentences>. Check especially: <the ways it
    could lose the user's data or do the wrong thing>.

    Run `planner check` and export the tree. Check that the plan's
    `plant` lines catch the bugs most likely to hurt, and write a
    line for each claim you doubt.

## Skill review

Start one per skill, all at once, after the build. Each gets the same
change description and its own focus:

    Review a code change before its pull request opens.
    Applicable skills: review-plan, <this skill>.

    Change: branch <branch>, worktree <path>, `git diff <parent>
    HEAD`. Plan: <path>. Known: <findings to ignore, and why>.
    Test results: <suites summary>.

    Focus: <see below>.

- `review-plan`: data safety. Rehearse the change by hand with a
  scratch home and state folder. Can the user's data be lost, sent
  twice, or changed unseen?
- Each language skill: idiom, names, error handling, package
  boundaries, the race detector or type checker results in the
  suites summary, and `plant` lines for claims the plan's lines miss.
- `code-documentation`: every comment and doc line added or touched,
  as keep, shorten (with the exact text) or cut, with a reason. Text
  in touched code that became false, and approved text that needs a
  reword.
- `behavior-focused-testing`: for each test, the bug it catches and
  whether that bug is likely or costly. Low-value tests are required
  corrections, and so is untested behavior that is likely and costly.
  To delete a test, name it and the test that still covers its
  behavior.

The `code-documentation` and `behavior-focused-testing` reviewers
count as their skills' fresh checks only if they never see the plan
or the design. Leave `Plan: <path>` out of their briefs. Give the
`behavior-focused-testing` reviewer the task in two sentences instead.

## Revision message

Start a new planner with this whenever the plan needs changes, such
as after a review round or a design check. Before the build, leave
out the part in brackets.

    Revise the implementation plan <plan path>.
    Applicable skills: create-plan, <domain skills>,
    code-documentation, behavior-focused-testing. Load each first and
    report any that are unavailable. Follow create-plan's "When you
    are a delegated agent" section. Read `Projects/AGENTS.md` in the
    vault before writing there.

    [<Plan> is built: branch <branch>, HEAD <commit>. Do not touch
    the worktree.] Revise the plan only, and apply only the items
    below; the reviewers' reports in <report files> give their
    evidence. Design: <design digest>.
    `planner check` must pass. Run no test suite or linter, since a
    script runs them once on the revised plan. Update the `plant` lines
    a fix changes, without running them.

    Required: <numbered, each with evidence and the fix>.
    Decided: <optional items you chose, and how reviewers' conflicts
    were settled>. Record each decision in the plan's
    `## Notes for agents`.

    Report: new size, changed `plant` lines,
    `planner check` result, and anything not applied and why.

## Local model

A local model runs only a job a script checks: tests and linters,
`plant` lines, or the checks before a merge. build.md's
`suites.sh` and `plants.sh` and the pull-request skill's
`merge-check.sh` are such scripts. Plans, design
checks, reviews, PR text and decisions stay on cloud models, and you
build and ship. Write the job as one script that sends long output to
a log and writes a summary file whose last line is `done`. The model
gets no rules file, only this brief:

    Run this one command: `bash <script>`. Then print <summary file>
    exactly. Touch no other file and run nothing else. Never answer
    an approval request.

Read the summary file yourself. The job counts only when its last
line is `done`; never trust the model's own words about it.
