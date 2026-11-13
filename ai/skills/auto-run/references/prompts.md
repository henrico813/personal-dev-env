# Prompts for a run's agents

Fill each `<...>`. Every brief names the skills the agent must load,
and you load them too. Start every brief with "Read <rules file>
first and follow it for the whole task. Write your full report to
<report file>." `<rules file>` is the run's `common.md`, from step 3
of SKILL.md's auto hub; it holds the rules every agent gets and the
ones reviewers also get. `<design digest>` is the PR's file from
build.md's Write a design digest; for a card with no design, give
the card instead and leave out the Dictionary sentence.
`<suites summary>` is the PR's `<PR folder>/suites.txt`, from
build.md's Run the suites once per plan version, for the plan
version the agent reads.
`<Write in parts>` is the block in Write in parts below.
`<report file>` stands for a new file in the PR folder, such as
`<PR folder>/plan-review-1.md`. The first planner starts before that
folder exists, so give it a file in a folder you make with `mktemp -d`.

## Contents

- Planner
- Write in parts
- Next part
- Fresh plan review
- Code review
- Revision message

## Planner

    Write an implementation plan for <PR name>.
    Applicable skills: create-plan, <domain skills>,
    code-documentation, behavior-focused-testing. Load each first and
    report any that are unavailable. Follow create-plan's "When you
    are a delegated agent" section. Read `Projects/AGENTS.md` in the
    vault before writing there.

    Base: <branch> at <full commit>; for a child of an unbuilt
    parent, the commit of the parent's planned tree. Read: <roadmap
    page or card> and <parent plans>. Design: <design digest>, which
    holds the design passages this PR touches, the Dictionary and the
    design-edits entries they point to. Use the words the Dictionary
    defines in names and comments. Scope: <what this PR does and does
    not do>.

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

    <Write in parts>

Every revision goes to a new planner, with the revision message
below. The plan's `## Notes for agents` holds every decision, so the
new planner needs no earlier session.

## Write in parts

Add this to the end of every planner brief: the first planner, each
next part and each revision.

    Write the plan in parts, one part per planner. After each section
    or plan step you finish, run `pde-context --context`. It prints
    your context, such as `86k of 1M (9%)`. Once the first number is
    past 150k, stop. Before you stop, write each decision you made
    into the plan's `## Notes for agents`. Then end your report with
    `Left:` and what is left, such as "steps 5 to 9 and
    Verification". `planner check` fails until the last part is
    written, which is expected. When the command prints
    `no session ID` instead of a number, stop after every five plan
    steps.

## Next part

When a planner's report ends with `Left:`, start a new planner with
this. Repeat until a report has no `Left:` line.

    Continue the implementation plan <plan path>.
    Applicable skills: create-plan, <domain skills>,
    code-documentation, behavior-focused-testing. Load each first and
    report any that are unavailable. Follow create-plan's "When you
    are a delegated agent" section. Read `Projects/AGENTS.md` in the
    vault before writing there.

    The planner before you stopped at its context limit. What is
    left: <the Left: list>. Read <roadmap page or card> and <parent
    plans>. Design: <design digest>. Read the plan as written so far
    with `rg` and `sed -n`, and write what is left. Keep the
    decisions in its `## Notes for agents`; change one only on
    evidence, and say so in your report.

    Report: plan paths, `planner check` result, changed lines (code,
    tests, other), choices you made, and anything that contradicts the
    design.

    <Write in parts>

## Fresh plan review

For a T1 plan, start one reviewer with this brief as it stands. For
T2 and T3, start one reviewer per skill, as pr-loop.md's Review tiers
says, and change the brief's Applicable skills line for each. The
`review-plan` reviewer gets `review-plan` and the language skills.
Each other reviewer gets `review-plan` and its own skill, such as
`review-plan, code-documentation`:

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

## Code review

Start the code review's reviewers that pr-loop.md's Review tiers
names for the PR's tier, all at once, after the build. Each gets the
same change description and its own focus. When one
reviewer covers several skills, name each on its Applicable skills
line and give it each skill's focus:

    Review a code change before its pull request opens.
    Applicable skills: review-plan, <this skill>.

    Change: branch <branch>, worktree <path>, `git diff <parent>
    HEAD`. Plan: <path>. Known: <findings to ignore, and why>.
    Test results: <suites summary>.

    Focus: <see below>.

- `review-plan` with the language skills: idiom, names, error
  handling, package boundaries, the race detector or type checker
  results in the suites summary, and `plant` lines for claims the
  plan's lines miss. For T2 and T3, also data safety: rehearse the
  change by hand with a scratch home and state folder. Can the
  user's data be lost, sent twice, or changed unseen?
- `code-documentation`: every comment and doc line added or touched,
  as keep, shorten (with the exact text) or cut, with a reason. Text
  in touched code that became false, and approved text that needs a
  reword. Never ask to cut a Python docstring's Google-style `Args:`,
  `Returns:` or `Raises:` sections.
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

    <Write in parts>
