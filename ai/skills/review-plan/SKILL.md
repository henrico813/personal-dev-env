---
name: review-plan
description: 'Review any change, such as an implementation plan, pull request, commit, commit range, or diff, for correctness, simplicity, and completeness, including code review, for requests like "review this plan", "review PR 253", or "review this branch"; for authoring a plan use create-plan, and for execution use implement-plan.'
metadata:
  pde-workflow: "true"
---

# Review Plan

Review a change: an implementation plan before implementation begins, or
code in a pull request, commit, commit range, or diff. Scale review
effort to the change's complexity and focus on concrete correctness,
unnecessary complexity, completeness, and reviewer intent.

## When to use this skill

Use this skill to review any change for correctness, simplicity, and
completeness: an implementation plan, a pull request, a commit or commit
range, a branch, or a diff. Do not use it for authoring a new plan or
executing one; use create-plan for authoring and implement-plan for
execution instead.

## Initial Response

When this command is invoked:

1. If a plan path, PR number or URL, commit, commit range, branch, a
   diff, or uncommitted changes are provided, read the change fully and
   begin.
2. If none is provided, ask for it and stop.

## Arguments

Take the task from the user's request or command arguments.

## Process

### Step 1: Read and Understand the Change

1. Read the complete change under review.
2. Read directly referenced requirements, identify affected domains, and load
   matching skills before broader repository review.
   Load `code-documentation` when the change touches source code or
   tests.
3. For a plan, read files named in its implementation and verification
   diffs. For code, read the diff and the changed files whole; for a
   deleted file, confirm nothing still refers to it instead. For a
   branch, compare against the base the user gives or the merge base
   with the default branch.
   For a PR with several commits, review the combined diff and note any
   commit that undoes another. For stacked PRs, check that the child is
   based on the current parent and does not edit files the parent
   deletes.
   Treat a PR description as requirements and check its Testing claims
   where practical.
4. Identify proposed changes, affected files, success criteria, and for
   a plan, sequencing.

### Step 2: Apply Review Principles

Always assess:

- Module depth: deep modules with simple interfaces, not shallow wrappers.
- Information hiding: design decisions remain inside their owning modules.
- Complexity direction: implementations absorb complexity instead of callers.
- Change amplification: changes do not ripple across unrelated files.
- Present need: new abstractions, wrappers, interfaces, and configuration are
  justified by a current requirement, invariant, or testing boundary.
- Scope control: unrelated cleanup and speculative extensibility stay optional.
- Implicit assumptions: unsupported beliefs are surfaced and checked.
- Bug potential: edge cases, error handling, concurrency, and failure modes.
- Completeness: callers, tests, config, docs, migrations, and verification are
  covered where relevant.
- Documentation quality: changed comments, docstrings, and test explanations
  remain accurate and proportional, important behavior is not left needlessly
  implicit, and obvious code does not receive mandatory prose.
  For a plan that changes comments, check that comments come after
  behavior and names, and that the owner's comment review comes last.
- Plan layout: for a plan, check create-plan's Plan Layout and its
  one-line-per-paragraph rule: owner-facing sections on top, agent-only
  detail in a final `## Notes for agents`, and no Decisions summary on
  top.

Treat text beyond the change reference as additional review criteria.

### Step 3: Choose Review Depth

Use the lightest review that reliably assesses the change. The complete
quality review is mandatory; delegation is conditional.

- For a bounded, familiar change, review directly in one pass.
- For a broad, risky, or cross-cutting change, launch focused reviewers in
  parallel for architecture/simplicity, bugs/failure modes, and
  completeness/integration.
- Add a specialist only when a domain materially benefits from it, such as
  security, persistence, concurrency, or migration behavior.
- When you are a delegated reviewer, review directly and do not launch
  further reviewers; you are the fresh-context test review that
  `behavior-focused-testing` asks for, so do not start another.

Every review must:

- remain read-only unless the user explicitly requests a review artifact; do
  not modify the change, source, tests, config, or repository state
- capture repository status before and after review and report any unexpected
  mutation instead of silently cleaning it up; running tests stays
  read-only when done in a scratch copy or without writing tracked files
- compare each diff with current source and directly affected callers,
  tests, config, and integration points
- confirm code is complete, applicable, free of placeholders, and limited to
  requested scope
- evaluate unnecessary abstractions, error handling, failure modes, and
  meaningful verification
- treat missing, placeholder-only, or non-behavioral verification that cannot
  prove the definition of done as a required correction, not an optional test
  improvement; when practical, check key test claims by mutation in a
  scratch copy
- check planned or changed tests against `behavior-focused-testing`;
  treat low-value tests
  as required corrections, just like missing tests, and do not request tests
  that would be low value
- cite concrete `file:line` evidence for repository-specific concerns
- distinguish a demonstrated problem from a preference or optional improvement
- avoid proposing unrelated cleanup

Every delegated reviewer must receive the change, user constraints,
reviewer preferences, and exact names of applicable skills, which may
include `review-plan`. Require it to load available
skills before review and report unavailable required skills.

Assign source-documentation findings to the completeness/integration review
when delegation is used; do not add a separate documentation reviewer loop.

### Step 4: Synthesize

1. Compile findings supported by the change or repository evidence.
2. Prioritize by severity, confidence, and impact, not repetition count.
3. Categorize findings as:
   - **Required corrections**: correctness, requirement, compatibility,
     verification, or unjustified-complexity issues to fix before
     implementation or merge.
   - **Optional suggestions**: useful improvements not required for this change.
4. Identify affected plan sections or files so revision can preserve
   accepted work.
   Report docs the change makes false without editing; if the owner
   deferred them, list them under Revision Scope instead.
5. Before presenting, confirm every required correction includes concrete
   evidence and every verification gap is classified by whether the definition
   of done remains unproven.

### Step 5: Present Findings

Present findings directly unless the user requests a review artifact. When an
artifact is requested, write only that exact destination and preserve every
other file:

```text
## Review: [Name]

### Required Corrections
- [Problem] - [why it matters] - [file:line evidence]

### Optional Suggestions
- [Improvement] - [benefit without expanding required scope]

### Revision Scope
- [Plan sections, files, or decisions that need to change]
- [Important decisions that should remain unchanged]

### Verdict
Plan: **[READY TO IMPLEMENT / NEEDS UPDATES / BLOCKED]**
Code: **[APPROVE / REQUEST CHANGES]**
```

Give only the matching verdict line: Plan for a plan, Code for code.

## Guidelines

1. Scale reviewer count instead of always spawning three agents.
2. Cite repository evidence for concrete concerns.
3. Prefer demonstrated correctness and simplicity issues over speculation.
4. Keep optional improvements separate from required corrections.
5. Reward the smallest set of edits needed for a sound change.
6. Preserve user-approved decisions unless new evidence invalidates them.
7. Read enough complete context to avoid partial-file errors.
