---
description: Clean up a completed plan worktree and finish end-of-change housekeeping
---

# Cleanup Plan

You are tasked with cleaning up after a completed implementation workflow. Your job is to verify that the plan worktree is safe to tear down, confirm the main checkout is healthy, and close out any remaining housekeeping without losing work.

## Getting Started

When invoked:

1. Determine the target worktree from the supplied plan path, explicit worktree path, branch name, or current directory.
2. If the target is ambiguous, stop and ask for the exact plan or worktree to clean up.
3. Read any referenced plan or note completely before making cleanup decisions.
4. Create a todo list to track the cleanup tasks.

## Cleanup Philosophy

Cleanup is a safety workflow, not a convenience command. Your job is to leave the repo in a predictable state without deleting unfinished work.

- Prefer preserving work over aggressively removing directories.
- Treat uncommitted or untracked implementation changes as a stop condition.
- Treat the main checkout as a protected baseline that must be inspected before teardown.
- Update plan and documentation status before removing the worktree that produced them.
- Explain what you verified, what you cleaned up, and what still needs human attention.

## Cleanup Procedure

### 1. Identify the worktree and branch

Before changing anything:
- Confirm the target worktree path.
- Confirm which branch is checked out in that worktree.
- Confirm whether the worktree is a plan worktree or an ad hoc checkout.
- If a plan note exists, identify whether it should be marked complete or updated with follow-up status.

### 2. Inspect safety conditions

Run these checks before removing a worktree:
- Check `git status --short` inside the target worktree.
- Check for untracked files that may represent unfinished work.
- Check whether the branch has commits not present on the main checkout.
- Inspect the main checkout and confirm it is in a healthy state.
- Check whether any docs, plans, or research notes still need status updates.

If you find uncommitted work, STOP and report it using this format:

```text
Cleanup blocked

Worktree: [path]
Branch: [branch]
Reason: [uncommitted changes, untracked files, docs not updated, or other blocker]

Required next action:
- [what the human should review or decide]
```

Do not remove the worktree until the blocker is resolved or the user explicitly approves a different action.

### 3. Finish housekeeping

If the worktree is safe to clean up:
- Update the plan note status if the workflow requires it.
- Update related documentation or follow-up notes if they are part of the change narrative.
- Summarize any follow-up work that should remain open after cleanup.
- Confirm whether the branch should be kept for PR follow-up or is safe to remove with the worktree.

### 4. Remove the worktree safely

Only after the safety checks pass:
- Remove the target worktree with `git worktree remove`.
- Run `git worktree prune`.
- Confirm the removed worktree no longer appears in `git worktree list`.
- Confirm the main checkout is still clean and on the expected branch.

## Response Format

When cleanup succeeds, report:

```text
Cleanup complete

Worktree removed:
- [path]

Verified:
- [main checkout state]
- [branch state]
- [plan/doc status updates]

Follow-up:
- [anything still requiring human action, or "None"]
```

When cleanup is blocked, report the blocker clearly and stop.

## Important Guidelines

1. Never remove a worktree that still contains uncommitted or unexplained files.
2. Never assume plan or documentation status is already correct; check it.
3. Prefer explicit verification over inference.
4. Keep the output concise, but include enough detail for a reviewer to understand what changed and what was verified.
