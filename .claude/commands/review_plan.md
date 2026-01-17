---
description: Review an implementation plan using parallel agents for simplicity, bugs, maintainability, and codebase consistency
model: opus
argument-hint: <plan-path> [additional guiding principles...]
---

# Review Plan

You are tasked with reviewing an implementation plan before implementation begins. Use parallel agents to evaluate the plan for simplicity, bugs, maintainability, and codebase consistency.

## Initial Response

When this command is invoked:

1. **If a plan path is provided via $ARGUMENTS**: Read it fully and begin the review process
2. **If no plan path provided**, respond with:
```
I'll review an implementation plan. Please provide the path to the plan file.

Example usage:
- /review_plan docs/plans/2025-01-15-feature-name.md
- /review_plan docs/plans/2025-01-15-feature-name.md with focus on security and performance
```

Then wait for the user's input.

## Arguments

**$ARGUMENTS**: $ARGUMENTS

## Process Steps

### Step 1: Read and Understand the Plan

1. **Read the plan FULLY** - no limit/offset parameters
2. **Read any referenced files**:
   - Original tickets or requirements documents
   - Related research documents
   - Files mentioned in "Changes Required" sections
3. **Identify key elements**:
   - Proposed changes and files to be modified
   - Implementation phases
   - Success criteria
4. **Create a review checklist** using TodoWrite to track review tasks

### Step 2: Parse Guiding Principles

**Default focus areas** (always apply):
- Simplicity - is the plan over-engineered?
- Bugs - potential issues in proposed changes
- Maintainability - long-term health of the codebase
- Codebase consistency - follows existing patterns

**Additional principles**: If $ARGUMENTS contains text beyond the plan path (e.g., "with focus on security"), extract those as additional review criteria.

### Step 3: Spawn Parallel Review Agents

Launch 3 agents IN PARALLEL (single message, multiple Task calls):

1. **codebase-analyzer**:
   - Analyze the files the plan proposes to modify
   - Check for potential bugs, edge cases, logical issues in the proposed approach
   - Evaluate against the guiding principles
   - Cite specific file:line references

2. **codebase-pattern-finder**:
   - Find existing patterns in the codebase related to what the plan implements
   - Verify the plan follows established conventions
   - Check for consistency with how similar features are implemented
   - Flag deviations from existing patterns

3. **codebase-locator**:
   - Locate related code the plan might have missed
   - Find integration points and dependencies
   - Identify affected areas not mentioned in the plan
   - Check for code that should be updated but isn't listed

Each agent prompt should include:
- Summary of what the plan proposes to change
- The guiding principles to evaluate against
- Instructions to cite specific file:line references
- Focus on finding issues, not documenting what's fine

### Step 4: Wait and Synthesize

1. **WAIT for ALL agents to complete** before proceeding
2. **Compile findings** from each agent
3. **Identify overlapping concerns** - issues flagged by multiple agents are higher priority
4. **Categorize by severity**:
   - **Blocking**: Issues that must be fixed before implementation
   - **Suggestions**: Non-blocking improvements

### Step 5: Present Review Findings

Present findings directly to the user (do NOT write to a file):

```
## Plan Review: [Plan Name]

### Potential Issues
[From codebase-analyzer]
- [Bug or edge case with file:line reference]
- [Logical issue in proposed approach]

### Codebase Consistency
[From codebase-pattern-finder]
- [Pattern deviation or convention violation]
- [Inconsistency with existing implementation]

### Missing Considerations
[From codebase-locator]
- [Related code not addressed in plan]
- [Integration point or dependency missed]

### Consolidated Concerns

**Blocking:**
- [Issue that must be fixed before implementation]

**Suggestions:**
- [Non-blocking improvement]

### Verdict
**[READY TO IMPLEMENT / NEEDS UPDATES / BLOCKED]**

[If NEEDS UPDATES: List specific changes required]
[If BLOCKED: Explain what must be resolved first]
```

## Important Guidelines

1. **Spawn agents in parallel** for efficiency - use a single message with multiple Task calls
2. **Wait for ALL agents** before synthesizing findings
3. **Be specific** - always cite file:line references for concerns
4. **Don't block on minor issues** - note them as suggestions but don't mark BLOCKED
5. **Focus on the guiding principles** - simplicity, bugs, maintainability, consistency
6. **Present findings directly** - do NOT write the review to a file
7. **Read files FULLY** - never use limit/offset parameters

## Example Usage

```
/review_plan docs/plans/2025-01-15-auth-feature.md
/review_plan docs/plans/2025-01-15-auth-feature.md with focus on security
/review_plan docs/planning/active/2025-01-15-new-api.md with focus on performance and backwards compatibility
```
