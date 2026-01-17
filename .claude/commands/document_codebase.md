---
description: Complete documentation workflow - review gaps, implement fixes, commit changes
argument-hint: [optional: specific file or directory to focus on]
---

## Context

Current working directory: !`pwd`
Recent changes: !`git log --oneline -5`
Documentation files: !`find . -name "*.md" -type f 2>/dev/null | head -20`

## Your Task

$ARGUMENTS

Execute a complete documentation workflow in three phases:

## Phase 1: Documentation Review

Use the **docs-reviewer** agent to analyze the current state of documentation:
- Identify code changes that need corresponding documentation updates
- Find documentation that has become outdated
- Suggest areas where new documentation should be added
- Review documentation quality and consistency

Provide the agent with:
- The scope (from $ARGUMENTS if provided, otherwise entire project)
- Recent git changes for context
- Current documentation structure

Wait for the review to complete before proceeding.

## Phase 2: Implement Changes

Use the **docs-writer** agent to execute the review recommendations:
- Update existing documentation files
- Create new documentation where needed
- Fix broken links and references
- Ensure consistent formatting and quality

Pass the complete findings from Phase 1 to this agent.

Wait for implementation to complete before proceeding.

## Phase 3: Commit Changes

After documentation updates are complete:

1. Review the changes made with `git status` and `git diff`
2. Stage documentation changes with `git add`
3. Create a conventional commit with message format: `docs: <brief description of changes>`
4. Confirm commit success with `git status`

## Guidelines

- If $ARGUMENTS specifies a file or directory, focus the entire workflow on that scope
- Otherwise, perform a comprehensive documentation review of the project
- Execute phases sequentially - each must complete before the next begins
- Present a summary of all changes at the end
