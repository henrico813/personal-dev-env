---
description: Create detailed implementation plans through the shared Go planner CLI
---

# Create Plan

You are tasked with creating detailed implementation plans that are grounded in the actual codebase and ready for execution.

Your default behavior is:

1. Read all provided context fully.
2. Research the relevant code, tests, config, and docs.
3. Resolve uncertainty through investigation whenever possible.
4. Produce the full plan including every line needed for a code change.

Ask the user clarifying questions only when missing information would materially change the implementation, sequencing, or verification. Do not ask for approval on plan structure or phasing. The skill owns the structure.
  
## Initial Response

When this command is invoked:
  
1. **If parameters were provided**:
```text
I'll help you create a detailed implementation plan.

Please provide:
1. The task or ticket description, or a path to the design doc / issue
2. Any constraints or requirements that materially affect implementation
3. Links or paths to related docs, previous plans, or prior implementations

I'll research the relevant code and produce a concrete implementation plan.

Tip: You can invoke this command with a file directly: `/create_plan docs/design-feature-name.md`
```

2. **If no parameters were provided**:
   - If a file path, ticket reference, or document path was provided, read it fully.
   - Begin research immediately.

## Non-Negotiable Rules

- Read every mentioned file fully before drafting the plan.
- Research the relevant code, tests, config, and documentation before drafting the plan.
- Do not draft the final plan until research is complete.
- If blocking questions remain after research, ask only those questions and stop.
- Use exactly the required headings and heading order in the final plan unless the user explicitly asks for a different format.
- Do not add extra sections unless the user explicitly asks for them.
- Keep the final plan actionable. The output is an implementation issue, not a design brainstorm.
- Prefer `make` commands in verification when suitable targets exist. If no suitable `make` target exists, say so and use the direct command.
- Exact code in code blocks must be provided for all implementation and verification steps. Do not omit any lines of code or commands. This is a requirement for the plan to be actionable, reviewable, and unambiguous.

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

### Step 3: Plan Structure Development

Once aligned on approach:

1. **Create initial plan outline**:
   ```
   Here's my proposed plan structure:

   ## Overview
   [1-2 sentence summary]

   ## Implementation Phases:
   2. [Phase name] - [what it accomplishes]
   3. [Phase name] - [what it accomplishes]
   4. [Phase name] - [what it accomplishes]

   Does this phasing make sense? Should I adjust the order or granularity?
   ```

### Step 4: Detailed Plan Writing

After structure approval:

1. Run `~/.cluade/bin/planner show-schema` to see the expected JSON
2. Read the **Example Template** below
3. Produce the expected plan JSON
4. Run `~/.cluade/bin/planner create` to output the rendered issue to the vault

Do not emit freeform markdown directly when the installed helper is available.

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
[Each step must contain every single line of code needed to make the change]

### 2. [Change description]

[...]

## Verification

[Test automation code or manual testing steps]

[Include explicit details like code to write, config to change, commands to run, etc.
[Each step must contain every single line of code needed to make the change]
```

Make sure the implementation and verification sections include explicit, 

### Step 5: Review

1. **Present the draft plan** and ask:
   - Are the steps properly scoped?
   - Are the success criteria specific enough?
   - Any technical details that need adjustment?
   - Missing edge cases or considerations?

2. **Iterate based on feedback** - be ready to:
   - Add missing phases
   - Adjust technical approach
   - Clarify success criteria (both automated and manual)
   - Add/remove scope items

3. **Continue refining** until the user is satisfied

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
   - Research actual code patterns using parallel sub-tasks
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

User: We need to add parent-child tracking for Claude sub-tasks. See docs/research-2025-01-08-parent-child-tracking.md
Assistant: Let me read that document completely first...

[Reads file fully]

Based on the document, I understand we need to track parent-child relationships for Claude sub-task events. Before I start planning, I have some questions...

[Interactive process continues...]
```

