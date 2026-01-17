---
description: Review an implementation plan for architecture, bugs, and completeness using parallel agents
model: opus
argument-hint: <plan-path> [additional guiding principles...]
---

# Review Plan

Review an implementation plan before implementation begins. Use parallel agents to evaluate architecture (SOLID, SoC), potential bugs, and completeness.

## Initial Response

When this command is invoked:

1. **If a plan path is provided via $ARGUMENTS**: Read it fully and begin the review process
2. **If no plan path provided**, respond with:
```
I'll review an implementation plan. Please provide the path to the plan file.

Example usage:
- /review_plan docs/planning/active/2025-01-15-feature-name.md
- /review_plan docs/planning/active/2025-01-15-feature-name.md with focus on security
```

Then wait for the user's input.

## Arguments

**$ARGUMENTS**: $ARGUMENTS

## Process Steps

### Step 1: Read and Understand the Plan

1. **Read the plan FULLY** - no limit/offset parameters
2. **Read any referenced files**:
   - Original tickets or requirements documents
   - Files mentioned in "Changes Required" or "Files to Modify" sections
3. **Identify key elements**:
   - Proposed changes and files to be modified
   - Implementation phases
   - Success criteria

### Step 2: Parse Guiding Principles

**Default focus areas** (always apply):
- SOLID principles - SRP, OCP, DIP violations
- Separation of Concerns - layer boundaries respected
- Bug potential - edge cases, error handling, failure modes
- Completeness - all affected files covered

**Additional principles**: If $ARGUMENTS contains text beyond the plan path (e.g., "with focus on security"), extract those as additional review criteria.

### Step 3: Spawn Parallel Review Agents

Launch 3 agents IN PARALLEL (single message, multiple Task calls):

1. **plan-architecture-reviewer**:
   - Evaluate SOLID compliance
   - Check separation of concerns
   - Assess coupling and scalability
   - Verify maintainability and testability

2. **plan-bug-reviewer**:
   - Anticipate runtime errors and edge cases
   - Check error handling coverage
   - Identify async pitfalls and race conditions
   - Validate input handling

3. **plan-completeness-reviewer**:
   - Find files the plan missed (dependents, tests, configs)
   - Verify all integration points covered
   - Check for missing migrations or documentation
   - Ensure test coverage addressed

Each agent prompt should include:
- Summary of what the plan proposes to change
- List of files to be modified
- The guiding principles to evaluate against
- Instructions to cite specific file:line references

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

### Architecture
[From plan-architecture-reviewer]
- [SOLID violation or SoC issue with file:line reference]
- [Coupling or maintainability concern]

### Potential Bugs
[From plan-bug-reviewer]
- [Critical] [Bug that will cause runtime failure]
- [Warning] [Edge case or error handling gap]

### Completeness
[From plan-completeness-reviewer]
- [Missing file that needs to be in plan]
- [Integration point not addressed]

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

1. **Spawn agents in parallel** - use a single message with multiple Task calls
2. **Wait for ALL agents** before synthesizing findings
3. **Be specific** - always cite file:line references for concerns
4. **Don't block on minor issues** - note them as suggestions
5. **Focus on the guiding principles** - SOLID, SoC, bugs, completeness
6. **Present findings directly** - do NOT write the review to a file
7. **Read files FULLY** - never use limit/offset parameters

## Example Usage

```
/review_plan docs/planning/active/2025-01-15-auth-feature.md
/review_plan docs/planning/active/2025-01-15-auth-feature.md with focus on security
/review_plan docs/planning/active/2025-01-15-new-api.md with focus on performance
```
