---
description: Create detailed implementation plans through the shared Go planner CLI
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
- Use the unique managed root printed by `surveil new task --task architecture`; do not create or reuse a shared task path.
- Avoid dependency, virtualenv, worktree, generated output, and cache directories unless explicitly requested.
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
3. Resolve plan, issue, doc, and vault references using the Planning Docs guidance in `ai/AGENTS.md`.
   - Use existing filesystem paths directly.
   - For existing plans, docs, and notes, use `pde vault locate --json --vault <selector> "<reference>"`.
   - Use `pde vault path <selector>` only when determining the destination root for a new plan or when the user explicitly asks for a vault root.
   - Ask only on `ambiguous`, `not_found`, or setup `error`.
4. Identify the code paths, modules, tests, config, and docs that are likely to be affected.

### Step 2: Research the Codebase

For repo-backed implementation plans, use `surveil` as the default research workflow.

For requests that are not repo-backed, skip `surveil` and research the relevant code, tests, config, docs, or comparative material directly using the available read-only tools before drafting.

For this workflow, <task-context> is the user's request and every referenced document.

For this workflow, <evidence-review-agent> is the OpenCode `codebase-analyzer` agent.

## Detailed Surveil Research Instructions

1. Create the three Surveil tasks:
   - Before any Surveil command, run `failure_file="$(mktemp "${TMPDIR:-/tmp}/surveil-research-failure.XXXXXX")"` to reserve a unique fallback failure file.
   - Run `search_dir="$(surveil new task --task architecture)"`.
   - After root creation succeeds, run `rm -f "$failure_file"` and then `failure_file="$search_dir/failure.md"`.
   - Run `surveil new task --root "$search_dir" --task interfaces-data-state`.
   - Run `surveil new task --root "$search_dir" --task tests-verification`.
2. Populate `$search_dir/architecture/task.json`, `$search_dir/interfaces-data-state/task.json`, and `$search_dir/tests-verification/task.json` from <task-context>:
   - Set `summary` to the task-context title; if it has no title, use its first sentence verbatim.
   - Set `explicit_files` to only literal paths named by the task context, preserving first-seen order and removing exact duplicates.
   - Set `search_areas` to the smallest repo directories covering those paths and each task's focus; use `.` only when the intended scope is the repository root.
   - Treat every relative `explicit_files` and `search_areas` value as relative to the exact <repo> passed to `--repo`; recalculate them if <repo> changes.
   - Set `terms` to literal identifiers, filenames, path segments, commands, and feature names, de-duplicate case-insensitively, and do not invent synonyms.
3. Populate each task's `query` array with its ordered questions. Do not omit, reorder, combine, reword, or reuse question sets across tasks.
   - `architecture`:
     1. `How does the current command or request flow through this area?`
     2. `Which modules own this behavior, and where are their boundaries?`
     3. `Which callers and integration points would need to change?`
     4. `What orchestration or dependency direction must be preserved?`
     5. `Which files define the complete implementation path?`
   - `interfaces-data-state`:
     1. `Which structs, types, functions, and fields define this behavior?`
     2. `How does data enter, change, and leave this area?`
     3. `Which validation rules and invariants must be preserved?`
     4. `Which persistence, environment, filesystem, process, or API boundaries are involved?`
     5. `Which compatibility or migration concerns apply?`
   - `tests-verification`:
     1. `Which existing tests and fixtures cover this behavior?`
     2. `Which test helpers and patterns should new coverage follow?`
     3. `Which docs, config, commands, and CI targets affect this change?`
     4. `Which automated checks verify the implementation?`
     5. `Which behavior requires manual verification?`
4. Run `surveil index --repo <repo>`.
5. Run all three gather commands:
   - `surveil gather --repo <repo> --task-file "$search_dir/architecture/task.json" > "$search_dir/architecture/context.json"`
   - `surveil gather --repo <repo> --task-file "$search_dir/interfaces-data-state/task.json" > "$search_dir/interfaces-data-state/context.json"`
   - `surveil gather --repo <repo> --task-file "$search_dir/tests-verification/task.json" > "$search_dir/tests-verification/context.json"`
6. Launch all three research commands through parallel tool calls and wait for all three:
   - `surveil research --context "$search_dir/architecture/context.json" --trace-out "$search_dir/architecture/trace.json" > "$search_dir/architecture/report.json"`
   - `surveil research --context "$search_dir/interfaces-data-state/context.json" --trace-out "$search_dir/interfaces-data-state/trace.json" > "$search_dir/interfaces-data-state/report.json"`
   - `surveil research --context "$search_dir/tests-verification/context.json" --trace-out "$search_dir/tests-verification/trace.json" > "$search_dir/tests-verification/report.json"`
7. Proceed to step 8 only after all three tasks succeed; otherwise follow step 12.
8. Merge the reports directly with `surveil merge "$search_dir/architecture/report.json" "$search_dir/interfaces-data-state/report.json" "$search_dir/tests-verification/report.json" > "$search_dir/evidence.json"`.
9. Read `$search_dir/evidence.json` before additional repository research.
10. After successful evidence, run one <evidence-review-agent>:
    - Give it <task-context>, <repo>, and `$search_dir/evidence.json`.
    - Find required files or behavior missing from the evidence and correct assumptions not supported by direct file reads.
    - Check related callers, integration points, and existing patterns outside the searched areas.
    - Identify missing tests, fixtures, config, commands, CI checks, or manual verification.
    - Require read-only research with concrete `file:line` references and findings not already present in the evidence.
    - Save its final response verbatim as `$search_dir/manual-review.md`.
11. Verify new or conflicting findings from <evidence-review-agent> with direct file reads before continuing.
12. If task JSON or search-area validation fails, correct the input and rerun gather; input corrections do not consume the operational retry. If any other Surveil command fails, retry it once. If it still fails, write the failed stage to `$failure_file`, skip steps 8-9 only, run one <evidence-review-agent> using all step 10 review instructions with <task-context>, <repo>, and any available artifacts, save its response beside `$failure_file`, and verify new or conflicting fallback findings with direct file reads before continuing.
## Evidence Review Best Practices

Run one research task after reading merged evidence. Give the analyzer exact directories, require read-only tools and `file:line` references, wait for it to complete, and cross-check unexpected findings directly.

Example:
```python
task = Task("Review Surveil evidence", evidence_review_prompt)
```

Assistant: This is a repo-backed implementation plan, so I'll create three managed Surveil tasks, merge their reports, and use one <evidence-review-agent> to review the evidence before drafting.


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
3. For partial updates, run `planner inspect <plan.md>` to see the parsed plan JSON and `update_diff_expect` tokens.
4. Prefer `planner patch <plan.md> [<out.md>]` for transactional scalar, checklist, and `Update Diff` edits.
5. Use behavioral commands when the patch v1 surface does not cover the change; for same-file behavioral edits, `<out.md>` may equal `<plan.md>`.
6. Preserve the supported wrapped-issue frontmatter on same-path edits and validate the final markdown with `planner check <output.md> --json-errors`.

Do not emit freeform markdown directly when the installed helper is available.

#### Partial Updates

For targeted updates to an existing plan:

1. Run `planner inspect <plan.md>`
2. Prefer `planner patch <plan.md> [<out.md>]` for transactional scalar and checklist edits.
3. Fall back to behavioral commands such as `planner implementation step file-change add` when patch v1 does not cover the change.

Non-targeted sections are preserved byte-for-byte, including supported wrapped issue frontmatter.

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

2. **Be Directed**:
   - Begin research immediately when enough information is present to resolve inputs
   - Ask clarifying questions only when missing information would materially change implementation, sequencing, or verification
   - Do not pause for approval on plan structure or intermediate research updates

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

## Example Interaction Flow

```
User: /create_plan
Assistant: I'll help you create a detailed implementation plan...

User: We need to add parent-child tracking for agent sub-tasks. See docs/research-2025-01-08-parent-child-tracking.md
Assistant: Let me read that document completely first...

[Reads file fully]

Assistant: This is a repo-backed implementation plan, so I'll create three managed Surveil tasks, merge their reports, and use one <evidence-review-agent> to review the evidence before drafting.

[Interactive process continues...]
```
