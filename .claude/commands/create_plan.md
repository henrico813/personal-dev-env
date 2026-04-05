---
description: Create detailed implementation plans through codebase research and evidence-backed synthesis
---

# Implementation Plan

You are tasked with creating detailed implementation plans that are grounded in the actual codebase and ready for execution.

Your default behavior is:

1. Read all provided context fully.
2. Research the relevant code, tests, config, and docs.
3. Resolve uncertainty through investigation whenever possible.
4. Produce the full implementation plan in one response.

Ask the user clarifying questions only when missing information would materially change the implementation, sequencing, or verification. Do not ask for approval on plan structure or phasing. The skill owns the structure.

## Initial Response

When this command is invoked:

1. **If parameters were provided**:
   - If a file path, ticket reference, or document path was provided, read it fully.
   - Begin research immediately.

2. **If no parameters were provided**, respond with:
```text
I'll help you create a detailed implementation plan.

Please provide:
1. The task or ticket description, or a path to the design doc / issue
2. Any constraints or requirements that materially affect implementation
3. Links or paths to related docs, previous plans, or prior implementations

I'll research the relevant code and produce a concrete implementation plan.

Tip: You can invoke this command with a file directly: `/create_plan docs/design-feature-name.md`
```

Then wait for the user's input.

## Non-Negotiable Rules

- Read every mentioned file fully before drafting the plan.
- Research the relevant code, tests, config, and documentation before drafting the plan.
- Do not draft the final plan until research is complete.
- If blocking questions remain after research, ask only those questions and stop.
- If no blocking questions remain, produce the final implementation plan immediately.
- Use exactly the required headings and heading order in the final plan unless the user explicitly asks for a different format.
- Do not add extra sections unless the user explicitly asks for them.
- Keep the final plan actionable. The output is an implementation issue, not a design brainstorm.
- Prefer `make` commands in verification when suitable targets exist. If no suitable `make` target exists, say so and use the direct command.

## Workflow

### Step 1: Read and Gather Context

1. Read all files mentioned by the user fully.
2. Read any directly related design docs, research docs, prior implementation plans, and referenced JSON or data files fully.
3. Identify the code paths, modules, tests, config, and docs that are likely to be affected.

### Step 2: Research the Codebase

Research the codebase before planning. Use available read-only tools to inspect:

- implementation files
- tests
- configuration
- interfaces and contracts
- similar features or patterns
- related docs or notes if they affect execution

When available, parallelize independent research tasks. After research, read the most relevant discovered files fully before drafting.

Focus on:

- current behavior
- integration points
- invariants and constraints
- similar patterns to follow
- likely edge cases
- verification entry points

### Step 3: Determine Whether Clarification Is Required

After research, choose exactly one of these paths:

1. **If blocking questions remain**:
   Respond with:

   ```text
   Based on the task and my code research, I understand we need to [accurate summary].

   I've verified:
   - [finding]
   - [finding]
   - [constraint or edge case]

   Blocking questions:
   - [question whose answer would materially change implementation]
   - [question whose answer would materially change verification]
   ```

   Ask only questions that research could not resolve. Then stop and wait.

2. **If no blocking questions remain**:
   Produce the final implementation plan immediately.

## Final Plan Requirements

The final plan must be complete, actionable, and ready for execution.

Use exactly these headings, in this order:

1. `# [Title]`
2. `## Overview`
3. `## Definition of Done`
4. `### Goals`
5. `## Current State`
6. `## Module Shape`
7. `## Implementation`
8. `## Verification`
9. `## Evidence`

Do not add sections such as:

- Git workflow
- branch name
- PR instructions
- files to create
- files to modify
- commit sequence

Include those details only if the user explicitly requests them.

## Evidence Rules

Keep the plan prose readable. Do not attach full `path:line` citations directly to every sentence.

Instead:

- Support each distinct codebase claim with evidence labels such as `[E1]`, `[E2]`.
- Reuse evidence labels when multiple claims rely on the same source.
- Add a final `## Evidence` section that maps each label to one or more `path:line` references.
- If a claim cannot be supported by code or doc evidence, omit it from the final plan or resolve it during research. Do not present unsupported claims as fact.

## Brevity Rules

Keep the plan dense and reviewable.

- `Overview` must be 2 sentences max.
- `Definition of Done` intro must be 1-3 sentences max. Put specifics in `Goals`.
- `Current State` must use 3-6 bullets, not paragraphs.
- Each `Implementation` step may use at most 3 prose sentences before switching to bullets or a representative snippet.
- Any prose immediately above or below a representative code block must be 1 sentence max.
- Prefer bullets over prose whenever the content is factual, enumerative, or checkable.
- Do not repeat the same rationale across sections.

## Writing Guidelines

- Optimize for human review. The plan should be easy to scan and easy to execute.
- Use short connective prose in `Overview` and `Current State`.
- Keep the rest structured and concrete.
- Make the plan coherent and compact.
- Code includes implementation code, tests, config, contracts, and documentation when relevant.
- Be concise by default. Explain why changes exist, not just what they are.
- Present implementation steps in execution order.
- Each implementation step must be specific enough that a reviewer can evaluate it before any code is written.
- Prefer representative code snippets over full file dumps unless a very small module is being defined.
- Design for test. Tests should document the intended behavior and the important module boundaries.
- Do not leave open questions in the final plan.

## Final Plan Template

````markdown
# [Title]

## Overview

[2 sentences max describing what is changing and why.]

## Definition of Done

[1-3 sentences max describing what will be true when this issue is complete.]

### Goals

- [ ] [Concrete, verifiable outcome]
- [ ] [Concrete, verifiable outcome]
- [ ] [Concrete, verifiable outcome]

## Current State

[3-6 bullets describing the relevant current behavior and constraints.]
[Support each distinct factual claim with evidence labels such as `[E1]`, `[E2]`.]

Examples:
- [Current behavior] [E1]
- [Constraint or invariant] [E2]
- [Existing pattern to follow] [E3]

## Module Shape

```text
path/to/module_a.py        # purpose
path/to/module_b.py        # purpose
path/to/test_module.py     # purpose
```

## Implementation

### 1. [First change]

[Describe the code, config, tests, and docs to update.]
[Explain why this change exists and how it supports the Definition of Done.]

```python
# Representative snippet if useful.
# Keep snippets concise and intent-focused.
```

### 2. [Second change]

[Describe the next change in execution order.]
[Explain dependencies, integration points, and important edge cases.]

```python
# Representative snippet if useful.
```

### 3. [Third change]

[Describe final wiring, cleanup, follow-through, or verification-supporting changes.]

## Verification

### Automated Verification

- [ ] [Runnable command and expected result]
- [ ] [Runnable command and expected result]
- [ ] [Runnable command and expected result]

### Manual Verification

- [ ] [Human check tied to a goal]
- [ ] [Human check tied to a goal]
- [ ] [Human check tied to a goal]

## Evidence

- [E1] `path/to/file.py:10-25`
- [E2] `path/to/other_file.py:40-68`
- [E3] `path/to/test_file.py:12-30`
````

## Verification Standards

- Every goal must map to at least one verification item.
- Separate automated and manual verification.
- Prefer `make` targets when available.
- If no suitable `make` target exists, say so and provide the direct command.
- Use real working directories and runnable commands.
- Manual verification should check user-visible behavior, workflow behavior, or contract-level expectations.
- Avoid vague verification such as "review the code" or "confirm it looks right."

## Common Patterns

### For Database Changes

- start with schema or migration
- update storage interfaces
- update business logic
- expose through APIs or services
- update tests and fixtures
- verify forward execution and compatibility expectations

### For New Features

- research existing patterns first
- start with contracts or data model
- add backend or service logic
- expose interfaces or APIs
- add UI or adapter wiring last
- verify both happy path and edge cases

### For Refactoring

- document current behavior with evidence labels
- preserve behavior unless the issue explicitly changes it
- prefer incremental steps
- include tests that freeze important behavior before or during the refactor
- note compatibility constraints directly in the implementation steps

## Final Self-Check

Before sending the final plan, verify all of the following:

- all required headings are present
- no extra headings were added
- `Goals` has 8 items or fewer
- every distinct claim in `Current State` is supported by an evidence label
- no blocking questions remain
- implementation steps are ordered and actionable
- automated and manual verification are both present
- verification items map back to the goals
- `make` is used when suitable targets exist
- prose sections follow the brevity rules

If any item fails, revise the plan before responding.
