---
description: Export current plan to markdown for another AI to implement
argument-hint: [description]
---

# Export Plan

Output the current implementation plan to a markdown file for handoff.

## Arguments

**$ARGUMENTS**: $ARGUMENTS

## Behavior

1. **Determine plan content**:
   - If $ARGUMENTS is a path to an existing file, read and reformat it
   - Otherwise, export the plan from the current conversation
   - If no plan exists in conversation, ask what to export

2. **Generate frontmatter**:
   - Run: `~/.claude/scripts/plan-frontmatter.sh "<title>" "<description>"`
   - Use the output as the document header

3. **Determine output location**:
   - Use `docs/planning/active/YYYY-MM-DD-{description}.md`
   - Create directories as needed
   - Use $ARGUMENTS as description, or derive from plan title

4. **Handle conflicts**:
   - If file already exists, ask before overwriting

5. **Write and confirm**:
   - Write the plan (frontmatter + content)
   - Report the path

## Output Format

```markdown
[frontmatter from ~/.claude/scripts/plan-frontmatter.sh]

# [Plan Title] Implementation Plan

## Overview
[What and why]

## Current State
[What exists, constraints]

## Desired End State
[Success criteria, how to verify]

## Phase 1: [Name]
### Changes Required
[Files and code]

### Success Criteria
#### Automated
- [ ] [Commands to run]

#### Manual
- [ ] [What to verify]

---

## Phase 2: [Name]
...

## References
[Related files and context]
```

## Guidelines

- If no clear plan in conversation, ask for clarification
- Check for existing files before writing
- Read source files fully if reformatting an existing plan
