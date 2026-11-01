# Prompts for a roadmap run

Fill each `<...>`. Every brief names the skills the agent must load,
and you load them too. Add the user's style rules that apply, from
their notes or memory, such as comment length or call layout. Once
the user has given feedback on one PR, also add that feedback list
from the roadmap page.

## Contents

- Rules every agent gets
- Planner
- Fresh plan review
- Skill review
- Revision message

## Rules every agent gets

Paste these into every brief:

    The user is away; decide minor things yourself and say so.
    Never edit or commit a real `.code-comments.json`. Work only in
    folders you make with `mktemp -d`; never remove a fixed /tmp path
    or use another agent's folder.

Reviewers also get:

    Read-only: do not edit the worktree, commit, push or install. You
    are a delegated reviewer: review directly and start no other
    agents. Report in review-plan's format with file:line evidence.

## Planner

    Write an implementation plan for <PR name> of the roadmap.
    Applicable skills: create-plan, <domain skills>,
    code-documentation, behavior-focused-testing. Load each first and
    report any that are unavailable. Follow create-plan's "When you
    are a delegated agent" section. Read `Projects/AGENTS.md` in the
    vault before writing there.

    Base: <branch> at <full commit>; for a child of an unbuilt
    parent, the commit of the parent's planned tree. Read: <roadmap>,
    <parent plans>, and these sections of <design>: Dictionary,
    <headings this PR needs>. Read another design section only when
    your plan depends on it. Use the Dictionary's words in names and
    comments. Scope: <what this PR does and does not do>.

    Decisions, each labeled user-decided, orchestrator-proposed or
    inferred from code: <list>.

    Report: plan paths, `planner check` result, changed lines (code,
    tests, other), choices you made, and anything that contradicts the
    design.

Keep the planner's session ID. Every revision goes to that session,
so it keeps its context.

## Fresh plan review

    Review an implementation plan as a fresh reviewer.
    Applicable skills: review-plan, <domain skills>,
    code-documentation, behavior-focused-testing.

    Plan: <path> (base on its `Base commit:` line). Design: <path>,
    sections Dictionary and <the planner's headings>. Read another
    design section only when the plan depends on it.
    What it does: <two sentences>. Check especially: <the ways it
    could lose the user's data or do the wrong thing>.

    Run `planner check`, export the tree, run the tests and linters,
    and mutation-check three of the plan's claims.

## Skill review

Start one per skill, all at once, after the build. Each gets the same
change description and its own focus:

    Review a code change before its pull request opens.
    Applicable skills: review-plan, <this skill>.

    Change: branch <branch>, worktree <path>, `git diff <parent>
    HEAD`. Plan: <path>. Known: <findings to ignore, and why>.

    Focus: <see below>.

- `review-plan`: data safety. Rehearse the change by hand with a
  scratch home and state folder. Can the user's data be lost, sent
  twice, or changed unseen?
- Each language skill: idiom, names, error handling, package
  boundaries, the tests with the race detector or type checker, and
  mutation checks of the plan's claims.
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

Send to the planner's session once every reviewer has reported:

    <Plan> is built: branch <branch>, HEAD <commit>. <N> reviewers
    asked for changes. Revise the plan only; `planner check` must
    pass; rerun the tests and the affected mutations. Do not touch
    the worktree.

    Required: <numbered, each with evidence and the fix>.
    Decided: <optional items you chose, and how reviewers' conflicts
    were settled>.

    Report: new size, tests, mutations, `planner check` result, and
    anything not applied and why.
