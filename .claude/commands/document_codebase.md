---
description: Complete documentation workflow - review gaps, implement fixes, commit changes
argument-hint: [optional: specific file or directory to focus on]
---

## Context

Current working directory: !`pwd`
Recent changes: !`git log --oneline -5`
Directory structure: !`find . -type d -not -path '*/\.*' -not -path '*/node_modules/*' -not -path '*/dist/*' -not -path '*/__pycache__/*' 2>/dev/null | head -30`

## Your Task

$ARGUMENTS

Execute a documentation workflow with progressive disclosure in five phases:

## Phase 1: Directory Structure Scan

Scan the directory structure (not file contents) to understand the codebase layout:
- Identify directories with source code
- Note which directories already have README.md files
- Skip build artifacts (dist, node_modules, __pycache__, etc.)

If $ARGUMENTS specifies a scope, focus only on that area.

## Phase 2: Identify Documentation Needs

Based on the structure scan, identify:
- Directories that should have README.md but don't
- Prioritize by: code complexity, frequent modification, public interfaces

Do NOT read all markdown files upfront. Only note which directories need attention.

## Phase 3: Documentation Review

Use the **docs-reviewer** agent to analyze documentation for the identified directories:
- Check for outdated content in existing README.md files
- Identify documentation gaps
- Check file sizes for token concerns

Provide the agent with:
- The specific directories to review (from Phase 2)
- Recent git changes for context
- The scope (from $ARGUMENTS if provided)

Wait for the review to complete before proceeding.

## Phase 4: Implement Changes

Use the **docs-writer** agent to execute the review recommendations:
- Create README.md files in directories that need them
- Update existing documentation with corrections
- Fix broken links and references

The docs-writer will:
- Use the directory README.md template
- Ask before creating files over 500 lines
- Never add component docs to CLAUDE.md

Pass the complete findings from Phase 3 to this agent.

Wait for implementation to complete before proceeding.

## Phase 5: Commit Changes

After documentation updates are complete:

1. Review the changes made with `git status` and `git diff`
2. Stage documentation changes with `git add`
3. Create a conventional commit with message format: `docs: <brief description of changes>`
4. Confirm commit success with `git status`

## Guidelines

- Progressive disclosure: Scan structure first, read files only when needed
- If $ARGUMENTS specifies a file or directory, focus the entire workflow on that scope
- Execute phases sequentially - each must complete before the next begins
- Directory-local README.md files are the primary output (not CLAUDE.md)
- Present a summary of all changes at the end
