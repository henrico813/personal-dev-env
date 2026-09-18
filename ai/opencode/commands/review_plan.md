---
description: Review an implementation plan for correctness, simplicity, and completeness
---

# Review Plan

Review an implementation plan before implementation begins. Scale review
effort to the plan's complexity and focus on concrete correctness, unnecessary
complexity, completeness, and reviewer intent.

## Initial Response

When this command is invoked:

1. If a plan path is provided via `$ARGUMENTS`, read it fully and begin.
2. If no plan path is provided, ask for it and stop.

## Arguments

$ARGUMENTS

## Process

### Step 1: Read and Understand the Plan

1. Read the complete plan.
2. Read directly referenced requirements, identify affected domains, and load
   matching skills before broader repository review.
   Load `code-documentation` when the plan changes source code or tests.
3. Read files named in implementation and
   verification diffs.
4. Identify proposed changes, affected files, sequencing, and success criteria.

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

Treat text in `$ARGUMENTS` beyond the plan path as additional review criteria.

### Step 3: Choose Review Depth

Use the lightest review that can reliably assess the proposal. The complete
quality review is mandatory; delegation is conditional.

- For a bounded, familiar change, review directly in one pass.
- For a broad, risky, or cross-cutting change, launch focused reviewers in
  parallel for architecture/simplicity, bugs/failure modes, and
  completeness/integration.
- Add a specialist only when a domain materially benefits from it, such as
  security, persistence, concurrency, or migration behavior.

Every review must:

- remain read-only unless the user explicitly requests a review artifact; do
  not modify the plan, source, tests, config, or repository state
- capture repository status before and after review and report any unexpected
  mutation instead of silently cleaning it up
- compare each proposed diff with current source and directly affected callers,
  tests, config, and integration points
- confirm code is complete, applicable, free of placeholders, and limited to
  requested scope
- evaluate unnecessary abstractions, error handling, failure modes, and
  meaningful verification
- treat missing, placeholder-only, or non-behavioral verification that cannot
  prove the definition of done as a required correction, not an optional test
  improvement
- cite concrete `file:line` evidence for repository-specific concerns
- distinguish a demonstrated problem from a preference or optional improvement
- avoid proposing unrelated cleanup

Every delegated reviewer must receive the plan, user constraints, reviewer
preferences, and exact names of applicable skills. Require it to load available
skills before review and report unavailable required skills.

Assign source-documentation findings to the completeness/integration review
when delegation is used; do not add a separate documentation reviewer loop.

### Step 4: Synthesize

1. Compile findings supported by the plan or repository evidence.
2. Prioritize by severity, confidence, and impact, not repetition count.
3. Categorize findings as:
   - **Required corrections**: correctness, requirement, compatibility,
     verification, or unjustified-complexity issues to fix before implementation.
   - **Optional suggestions**: useful improvements not required for this change.
4. Identify affected plan sections so revision can preserve accepted work.
5. Before presenting, confirm every required correction includes concrete
   evidence and every verification gap is classified by whether the definition
   of done remains unproven.

### Step 5: Present Findings

Present findings directly unless the user requests a review artifact. When an
artifact is requested, write only that exact destination and preserve every
other file:

```text
## Plan Review: [Plan Name]

### Required Corrections
- [Problem] - [why it matters] - [file:line evidence]

### Optional Suggestions
- [Improvement] - [benefit without expanding required scope]

### Revision Scope
- [Sections or decisions that need to change]
- [Important decisions that should remain unchanged]

### Verdict
**[READY TO IMPLEMENT / NEEDS UPDATES / BLOCKED]**
```

## Guidelines

1. Scale reviewer count instead of always spawning three agents.
2. Cite repository evidence for concrete concerns.
3. Prefer demonstrated correctness and simplicity issues over speculation.
4. Keep optional improvements separate from required corrections.
5. Reward the smallest set of changes needed for a sound proposal.
6. Preserve user-approved decisions unless new evidence invalidates them.
7. Read enough complete context to avoid partial-file errors.
