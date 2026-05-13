---
name: create-plan
description: Use when the user asks to create a surveil-backed implementation plan through the shared Go planner CLI.
---

# Create Plan

You are tasked with creating detailed implementation plans that are grounded in the actual codebase and ready for execution.

Your default behavior is:

1. Read all provided context fully.
2. For repo-backed implementation planning, use surveil as the default research engine.
3. Resolve uncertainty through investigation whenever possible.
4. Produce the full plan including diffs of all lines needed for a code change.

Ask the user clarifying questions only when missing information would materially change the implementation, sequencing, or verification. Do not ask for approval on plan structure or phasing. The skill owns the structure.
  
## Initial Response

When this command is invoked:
  
1. **If parameters were provided**:
   - If a file path, ticket reference, or document path was provided, read it fully.
   - Begin research immediately.

2. **If no parameters were provided**:
```text
I'll help you create a detailed implementation plan.

Please provide:
1. The task or ticket description, or a path to the design doc / issue
2. Any constraints or requirements that materially affect implementation
3. Links or paths to related docs, previous plans, or prior implementations

I'll research the relevant code and produce a concrete implementation plan.

Tip: You can invoke this command with a file directly: `/create_plan docs/design-feature-name.md`
```

## Non-Negotiable Rules

- Read every mentioned file fully before drafting the plan.
- Research the relevant code, tests, config, and documentation before drafting the plan.
- For repo-backed implementation plans, treat `surveil` artifacts as required baseline inputs before broad manual repo research.
- Do not draft the final plan until research is complete.
- If blocking questions remain after research, ask only those questions and stop.
- Determine the final output path before running `planner new <output.md>` or scaffolding `surveil`.
- Write the final plan to its real destination, not a transient temp path.
- If the user says a legacy surface is being phased out, treat that surface as out of scope unless they explicitly request changes there.
- Use a unique per-plan temp artifact directory under `/tmp/opencode/`; do not reuse shared temp paths.
- Use exactly the required headings and heading order in the final plan unless the user explicitly asks for a different format.
- Do not add extra sections unless the user explicitly asks for them.
- Keep the final plan actionable. The output is an implementation issue, not a design brainstorm.
- Prefer `make` commands in verification when suitable targets exist. If no suitable `make` target exists, say so and use the direct command.
- Exact code in diff blocks must be provided for all implementation and verification steps. Do not omit any lines of code or commands. This is a requirement for the plan to be actionable, reviewable, and unambiguous.

## Workflow

First classify whether the request is a repo-backed implementation plan.

Treat the request as repo-backed and use `surveil` when any of these are true:
- the user wants an implementation plan for an existing repo feature, refactor, bug fix, or integration
- the plan depends on current code, tests, config, docs, or workflow behavior
- the request names files, modules, services, commands, or directories in this repo

Skip `surveil` when:
- the request is purely conceptual or comparative
- the user is brainstorming options without needing concrete repo research
- there is no meaningful local codebase surface to inspect

If any repo-backed trigger is present, do not fall back to manual-first research.

### Step 1: Read and Gather Context

1. Read all files mentioned by the user fully.
2. Read any directly related design docs, research docs, prior implementation plans, and referenced JSON or data files fully.
3. If the user references a vault-style path, resolve it with `pde vault locate --json --vault default "<reference>"`. If that returns `not_found`, retry once with `--vault any "<reference>"`. Treat `ambiguous`, `error`, and a final `not_found` as stop-and-ask states. Use the vault helper for lookup only; do not use it to create destination paths. Use `--query` only when you explicitly want note-content search.
4. If the request is repo-backed, let `<artifact-dir>` be a unique per-plan temp artifact directory under `/tmp/opencode/`, then scaffold a structured surveil task at `<artifact-dir>` before broad repo research.
5. Build the task using these mechanical rules:
   - Populate `<artifact-dir>/task.md` after scaffolding it.
   - `Summary`: copy the issue or document title verbatim; if there is no title, use the user's first sentence verbatim.
   - `Explicit Files`: include only literal file paths named by the user or directly named in the provided doc; preserve first-seen order and de-duplicate exact repeats.
   - `Search Areas`: if explicit files exist, derive parent directories from them, collapse nested directories to the shortest covering paths, preserve order, and cap the list at three; otherwise use only literal repo directories named in the request, or `.` if none are named.
   - `Query`: use this fixed ordered set of planning prompts:
     - `What is the current behavior in this area of the repo?`
     - `Where are the integration points or callers that would need to change?`
     - `What tests or test patterns already cover this area?`
     - `What docs, config, or commands affect this area?`
     - `How should this change be verified?`
   - `Terms`: include only literal identifiers, filenames, path segments, commands, and feature names copied from the request or source doc; de-duplicate case-insensitively and do not invent synonyms.
6. Identify the code paths, modules, tests, config, and docs that are likely to be affected.

### Step 2: Research the Codebase

For repo-backed implementation plans, use `surveil` as the default research workflow.

For requests that are not repo-backed, skip `surveil` and research the relevant code, tests, config, docs, or comparative material directly using the available read-only tools before drafting.

Capture artifacts with this fixed command pattern:

- `surveil new task <artifact-dir>`
- Populate `<artifact-dir>/task.md` using the mechanical rules above.
- `surveil gather --repo <repo> --task-file <artifact-dir>/task.md > <artifact-dir>/context.json`
- `surveil research --context <artifact-dir>/context.json --trace-out <artifact-dir>/trace.json > <artifact-dir>/report.json`

If `surveil` is unavailable, fails to run, or emits invalid artifacts, stop and ask the user how to proceed instead of silently reverting to broad manual repo research.

Consume artifacts in this order:

1. Read `<artifact-dir>/report.json` first and treat `result` as the default evidence outline.
2. Read `<artifact-dir>/trace.json` only when `open_questions` is non-empty, `blockers` is non-empty, explicit files are missing from relevant findings, or the report looks noisy.
3. Preserve `negative_evidence` and `open_questions` as planning inputs instead of smoothing them away.

Bound follow-up reading after `surveil`:

- prefer surfaced snippets over broad repo reads
- read full files only for the top surfaced file per query, with one extra file allowed only when the first file is insufficient for exact diff planning, capped at five total files
- when the query is about tests, docs, config, or verification, prefer surfaced files from that class before defaulting to implementation files

Do targeted follow-up investigation when `surveil` is weak or noisy, for example when:

- top ranked files are mostly docs, fixtures, generated files, or plumbing
- broad terms dominate `matched_from`
- explicit files are not preferred for queries they should answer
- a docs, config, or tests query returns only implementation snippets
- `open_questions` is non-empty
- `negative_evidence` conflicts with the expected repo surface

In those cases, refine with focused reads and searches rather than skipping `surveil`, and do not draft until each required planning query has evidence or explicit follow-up results.


### Step 3: Plan Structure Development

Once aligned on approach:

1. **Create the initial outline internally unless the user explicitly asked for interactive planning**:
   ```
   Here's my proposed plan structure:

   ## Overview
   [1-2 sentence summary]

   ## Implementation Phases:
   2. [Phase name] - [what it accomplishes]
   3. [Phase name] - [what it accomplishes]
   4. [Phase name] - [what it accomplishes]

   Use the smallest scope that satisfies the request and constraints.
   ```

### Step 4: Detailed Plan Writing

After research is complete:

1. Run `planner help` first; do not guess command shapes from memory.
2. For new plans, run `planner new <output.md>`.
3. Edit the markdown directly.
4. For partial updates, run `planner inspect <plan.md>` to see the parsed plan JSON.
5. Use behavioral commands such as `planner title set`,
   `planner implementation step summary set`, and `planner implementation step file-change diff set`; for behavioral edit commands,
   `<out.md>` may be the same path as `<plan.md>` for same-file updates.
6. Keep the legacy JSON flow only when intentionally needed: run `planner template --json > draft.json`, edit the draft JSON directly, run `planner check draft.json`, then run `planner create draft.json <output.md>`.

Do not emit freeform markdown directly when the installed helper is available.

#### Partial Updates

For targeted updates to an existing plan:

1. Run `planner inspect <plan.md>`
2. Run `planner help` to choose the matching behavioral edit command; for behavioral edit commands, `<out.md>` may be the same path as `<plan.md>` for same-file updates.
3. Use commands such as `planner dod goal set`, `planner implementation step file-change add`, or `planner verification automated remove`

Non-targeted sections are preserved byte-for-byte.

#### Example Template

```markdown
# [Title]

## Overview

[1-2 sentences: what and why]

## Definition of Done

[1-3 sentences max describing what will be true when this issue is complete.]

### Goals

- [ ] [Concrete, verifiable outcome]
- [ ] [Concrete, verifiable outcome]
- [ ] [Concrete, verifiable outcome]

### Current State

[3-6 bullets describing the relevant current behavior and constraints.]

Examples:
- [Current behavior]
- [Constraint or invariant]
- [Existing pattern to follow]

### Module Shape

[Directory and file structure of final outcome]

## Implementation

### 1. [Change description]

[Explicit details: code to write, config to change, commands to run, etc.
[Each step must contain every single line of code in a diff block needed to make the change]

### 2. [Change description]

[...]

## Verification

[Test automation code or manual testing steps]

[Include explicit details like code to write, config to change, commands to run, etc.
[Each step must contain every single line of code in a diff block needed to make the change]
```

Make sure the implementation and verification sections include explicit, 

### Step 5: Validate And Report

1. Run `planner check <output.md>`.
2. Fix any validation failures before reporting success.
3. Report the final output path, validation result, scope summary, and any blockers.

## Important Guidelines

1. **Be Skeptical**:
   - Question vague requirements
   - Identify potential issues early
   - Ask "why" and "what about"
   - Don't assume - verify with code

2. **Be Interactive**:
   - Don't write the full plan in one shot
   - Get buy-in at each major step
   - Allow course corrections
   - Work collaboratively

3. **Be Thorough**:
   - Read all context files COMPLETELY before planning
   - For repo-backed planning, use `surveil` before broad manual repo research
   - Research actual code patterns using parallel sub-tasks only after the initial `surveil` pass when additional investigation is still needed
   - Include specific file paths and line numbers
   - Write measurable success criteria with clear automated vs manual distinction
   - automated steps should use `make` whenever possible - for example `make -C myapp check` instead of `cd myapp && npm run fmt`

4. **Be Practical**:
   - Focus on incremental, testable changes
   - Consider migration and rollback
   - Think about edge cases
   - Include "what we're NOT doing"

5. **Track Progress**:
   - Use TodoWrite to track planning tasks
   - Update todos as you complete research
   - Mark planning tasks complete when done

6. **No Open Questions in Final Plan**:
   - If you encounter open questions during planning, STOP
   - Research or ask for clarification immediately
   - Do NOT write the plan with unresolved questions
   - The implementation plan must be complete and actionable
   - Every decision must be made before finalizing the plan

## Success Criteria Guidelines

**Always separate success criteria into two categories:**

1. **Automated Verification** (can be run by execution agents):
   - Commands that can be run: `make test`, `npm run lint`, etc.
   - Specific files that should exist
   - Code compilation/type checking
   - Automated test suites

2. **Manual Verification** (requires human testing):
   - UI/UX functionality
   - Performance under real conditions
   - Edge cases that are hard to automate
   - User acceptance criteria

**Format example:**
```markdown
### Success Criteria:

#### Automated Verification:
- [ ] Database migration runs successfully: `make migrate`
- [ ] All unit tests pass: `go test ./...`
- [ ] No linting errors: `golangci-lint run`
- [ ] API endpoint returns 200: `curl localhost:8080/api/new-endpoint`

#### Manual Verification:
- [ ] New feature appears correctly in the UI
- [ ] Performance is acceptable with 1000+ items
- [ ] Error messages are user-friendly
- [ ] Feature works correctly on mobile devices
````

## Common Patterns

### For Database Changes:
- Start with schema/migration
- Add store methods
- Update business logic
- Expose via API
- Update clients

### For New Features:
- Research existing patterns first
- Start with data model
- Build backend logic
- Add API endpoints
- Implement UI last

### For Refactoring:
- Document current behavior
- Plan incremental changes
- Maintain backwards compatibility
- Include migration strategy

## Sub-task Spawning Best Practices

When spawning research sub-tasks:

1. **Spawn multiple tasks in parallel** for efficiency
2. **Each task should be focused** on a specific area
3. **Provide detailed instructions** including:
   - Exactly what to search for
   - Which directories to focus on
   - What information to extract
   - Expected output format
4. **Be EXTREMELY specific about directories**:
   - Specify exact directory paths based on the project structure
   - Never use generic terms when specific paths are needed
   - Include the full path context in your prompts
5. **Specify read-only tools** to use
6. **Request specific file:line references** in responses
7. **Wait for all tasks to complete** before synthesizing
8. **Verify sub-task results**:
   - If a sub-task returns unexpected results, spawn follow-up tasks
   - Cross-check findings against the actual codebase
   - Don't accept results that seem incorrect

Example of spawning multiple tasks:
```python
# Spawn these tasks concurrently:
tasks = [
    Task("Research database schema", db_research_prompt),
    Task("Find API patterns", api_research_prompt),
    Task("Investigate UI components", ui_research_prompt),
    Task("Check test patterns", test_research_prompt)
]
```

## Example Interaction Flow

```
User: /create_plan
Assistant: I'll help you create a detailed implementation plan...

User: We need to add parent-child tracking for agent sub-tasks. See docs/research-2025-01-08-parent-child-tracking.md
Assistant: Let me read that document completely first...

[Reads file fully]

Assistant: This is a repo-backed implementation plan, so I'll build the fixed `surveil` task, capture `context`, `report`, and `trace`, and use `result` as the baseline evidence pack before drafting.

[Interactive process continues...]
```
