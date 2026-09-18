---
name: document-codebase
description: Use when the user asks to review, improve, or generate project documentation for a named area, current change, or explicit repository-wide audit.
---

# Document Codebase

## Resolve Intent and Scope

Classify the request before reading broadly:

- **Review or audit:** report findings without editing files.
- **Write, update, or fix:** make supported documentation-only edits.
- **Unclear intent:** ask whether the user wants findings or edits.

Resolve scope in this order:

1. Use paths, modules, or diff ranges named by the user.
2. Otherwise use the current task or change. Prefer staged and unstaged tracked
   files, then non-ignored untracked files. If the worktree is clean, use the
   branch diff only when its upstream or base branch is unambiguous.
3. Ask for scope when there is no clear current change.
4. Audit the full repository only when the user explicitly requests it.

Do not perform repository-wide README, docstring, function-length, or comment
inventories for a bounded request.

After resolving scope, load and follow `code-documentation` only when source
comments, docstrings, or test explanations are in scope. Follow repository and
language guidance for other documentation.

## Workflow

1. Read each scoped function, class, module, or test as a complete unit,
   including existing comments and docstrings outside the diff.
2. Read directly related tests, requirements, docs, and history only when they
   are needed to verify behavior or a stated reason.
3. Decide whether the right result is no change, a source-local explanation, an
   update to existing project documentation, or a broader structural document.
4. Apply the smallest supported change in edit mode.
5. In review mode, run one bounded read-only review, report supported findings
   with file and line references, and stop before editing.

Documentation-only work may change comments, docstrings, documentation examples,
and prose. Do not change behavior, public signatures, test names, test inputs,
or assertions unless the user explicitly asks for source changes.

If documentation disagrees with behavior, requirements are unclear, or a code
bug is possible, report the disagreement instead of documenting an assumption.

For bounded edit work, edit directly. For an explicitly broad edit, specialist
delegation is optional when the active harness provides it. Pass an `Applicable
skills:` line containing `code-documentation` only for source-documentation
work and require workers to load available listed skills before working. Do not
delegate review mode to an editing specialist.

For each review-mode pass, capture the current revision,
`git diff --binary --cached`, `git diff --binary`, `git status --porcelain=v1`,
and non-ignored untracked paths and content, plus supplied external files.
Compare every value immediately after the review. Any difference is an
unauthorized mutation: stop, report it, and do not use the review response.

## Verify and Report

Before reporting completion:

- read each changed explanation with the implementation or assertions it covers
- inspect the full diff and ensure no functional source change entered the task
- run documentation examples or relevant tests when the claims depend on them

A no-change result is valid. Fix supported issues once, then stop; do not begin
an open-ended prose-polishing loop.

Report the resolved scope, mode, findings or changes, verification performed,
and any unresolved documentation or behavior mismatch. Do not stage or commit
unless the user separately requested it.
