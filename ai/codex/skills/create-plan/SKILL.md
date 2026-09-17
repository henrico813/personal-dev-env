---
name: create-plan
description: Use when the user asks for a code-bearing implementation plan grounded in the current repository and reviewable before implementation.
---

# Create Plan

Create implementation proposals that are grounded in the current codebase,
easy for a human to review, and ready for execution once approved.

Your default behavior is:

1. Read the request and directly referenced context needed to identify the
   affected domains.
2. Load domain skills that materially constrain the implementation.
3. Read the relevant code and research only enough to resolve the decisions the
   proposal depends on; expand research when scope or uncertainty requires it.
4. Prefer existing repository patterns and present requirements over
   speculative abstractions or unrelated cleanup.
5. Produce a complete final plan with exact code diffs for implementation and
   verification.

Ask clarifying questions only when missing information would materially change
implementation, sequencing, or verification. Do not add planning ceremony when
the implementation direction is already clear.

## Invocation

Read the task, plan destination, constraints, and references from the user's
request. If no task was supplied, ask for the task, material constraints, and
related references, then stop. Read referenced files fully before substantive
research.

## Non-Negotiable Rules

- Load matching domain skills before making decisions they constrain. If
  research exposes another affected domain, load its skill and revisit affected
  decisions.
- When delegating, name every applicable loaded skill and require the subagent
  to load available skills before working.
- Read every mentioned file fully before the final proposal. Read directly
  related callers, tests, config, and docs when they affect the change.
- Scale research to the task. Do not run a fixed multi-agent or multi-track
  workflow merely because the task is repo-backed.
- Use `surveil` when it reduces uncertainty or efficiently finds cross-cutting
  dependencies; direct repository reads are valid for bounded changes.
- Before expanding a proposal, challenge new abstractions, wrappers,
  interfaces, configuration layers, and cleanup. Keep them only when a present
  requirement, invariant, or meaningful testing boundary justifies them.
- Exclude unrelated cleanup and speculative extensibility from required scope.
  Put genuinely useful extras in optional suggestions instead.
- A draft may expose an unresolved implementation decision early when expanding
  the wrong choice would create substantial rework. Label it as a draft; the
  final plan must resolve blocking decisions.
- Classify the operation as a new plan or revision. Reserve every new
  destination with `planner new`; it fails atomically rather than overwriting an
  existing plan.
- Determine the final output path before creating the plan and write the final
  plan to its real destination.
- If the user says a legacy surface is being phased out, treat it as out of
  scope unless explicitly requested.
- Avoid dependency, virtualenv, worktree, generated output, and cache
  directories unless explicitly requested.
- Keep the final plan actionable. It is a reviewable implementation proposal,
  not a design brainstorm.
- Prefer `make` commands in verification when suitable targets exist. Otherwise
  use the direct command.
- Exact code in diff blocks is required for every implementation and
  verification change in the final plan.

## Workflow

### Step 1: Read and Gather Context

1. Read all files mentioned by the user fully.
2. Read directly related requirements, design docs, prior plans, and data files.
3. Resolve plan, issue, doc, and vault references using the loaded shared
   Planning Docs guidance.
4. Load skills for the domains exposed by that context before broader source
   research or implementation decisions.
5. Identify the likely code paths, callers, tests, config, and docs.
6. Determine the output path and classify creation or revision.

### Step 2: Research the Codebase

Use the lightest research path that can support the proposal with evidence.

**Bounded, well-understood change**

- Read the named files and directly related callers, tests, config, and docs.
- Follow existing patterns before introducing a new one.
- Do not start Surveil or research subagents unless direct inspection leaves
  material uncertainty.

**Cross-cutting, unfamiliar, or uncertain change**

- Use `surveil --help` and relevant subcommand help rather than memorized
  command shapes.
- Create one focused Surveil task for the uncertainty to resolve. Add another
  only when it answers a distinct question inefficient to combine.
- Use a read-only research subagent only when evidence is incomplete,
  conflicting, or broad enough that an independent pass is valuable.
- Verify surprising or conflicting findings with direct file reads.

Research should answer only the questions needed to make the proposal
reviewable:

- What current behavior and invariants must remain true?
- Which implementation path and integration points actually change?
- Which existing pattern should this change follow?
- Which tests and verification commands prove the behavior?
- What compatibility, migration, persistence, process, or API boundaries apply?

Stop researching when those decisions are supported. More evidence is not a
goal by itself.

### Step 3: Choose the Smallest Implementation Shape

Before expanding the full diff, inspect the proposed module shape and main code
path.

- Prefer the smallest change that satisfies requirements and preserves current
  invariants.
- Reuse existing concrete types and flows when they provide the needed boundary.
- For every new abstraction, identify the present requirement or meaningful
  testing boundary that needs it. Remove hypothetical future flexibility.
- Keep unrelated cleanup out of the required proposal.

If one unresolved choice would cause substantial rework if wrong, expose only
enough representative code to make that choice reviewable. Do not expand the
rest until it is resolved. This is an optional fast-feedback path, not a
mandatory approval stage.

### Step 4: Write or Revise the Plan

1. Run `planner help` when you need the CLI; do not guess command shapes.
2. For a new plan, run `planner new <output.md>` to reserve the destination and
   scaffold the supported format. Stop if it reports that the destination
   exists.
3. After reservation, write or edit Markdown directly when that is simplest.
   The Markdown file is the human-facing source of truth.
4. For an existing plan, keep direct edits range-targeted and verify that
   wrapped-issue frontmatter and unrelated accepted sections remain byte-for-byte
   unchanged.
5. Before every revision to an existing fenced code diff, run
   `planner inspect <plan.md>` immediately, then use its current
   `update_diff_expect` token in a single-operation guarded `planner patch`
   `Update Diff`. Do not directly edit that diff or use an unguarded command.
6. Validate every final or revised proposal with
   `planner check <output.md> --json-errors`.

#### Revisions After Human Feedback

Treat review as a correction loop, not a restart:

1. Identify which decisions and sections the feedback invalidates.
2. Preserve unrelated accepted decisions and proposal sections.
3. Re-research only when feedback invalidates an assumption, reveals a missing
   dependency, or changes the affected surface.
4. Apply required corrections first. Keep optional improvements separate.
5. Show the complete proposal diff and reject collateral changes.
6. Re-run Planner validation.

### Step 5: Validate and Report

1. Run `planner check <output.md> --json-errors`.
2. Compare every proposed diff with current source. Confirm that it applies to
   the intended file, includes every required line without placeholders,
   follows repository patterns, and excludes unrelated work.
3. Perform the complete quality review directly. For broad, risky, or
   cross-cutting work, delegate focused architecture, bug, and completeness
   reviews in parallel and reconcile evidence-backed findings.
4. Fix validation or source-review failures before reporting success.
5. Report the output path, validation result, scope summary, and blockers.

## Review Principles

- Verify assumptions against code.
- Challenge unnecessary abstraction, hidden scope expansion, and speculative
  flexibility.
- Distinguish required corrections from optional improvements.
- Spend research effort according to uncertainty, blast radius, and
  unfamiliarity.
- Consider migration, rollback, failure modes, and edge cases when relevant.
- Final plans must resolve blocking questions and include complete
  implementation and verification diffs.
