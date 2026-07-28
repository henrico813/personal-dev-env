---
description: Clean up completed plan, worktree, branch, PR evidence, and main-checkout housekeeping
---

# Cleanup Plan

You are tasked with cleaning up after a completed implementation workflow. Your job is to verify that the plan worktree is safe to tear down, confirm the main checkout is healthy, and close out any remaining housekeeping without losing work.

User context:

$ARGUMENTS

## Getting Started

When invoked:

1. Inventory the plan, repository, Vibe run, worktree, branch, pull request, remote, and main checkout independently.
2. Use supplied plan paths, worktree paths, branch names, Vibe keys, pull request references, and the current directory as evidence.
3. Treat an artifact that was not supplied and is absent as a skipped cleanup action.
4. Treat an explicitly supplied artifact that cannot be found as a blocker or ambiguity that requires user input.
5. If identity is ambiguous, stop and ask for the exact plan, worktree, branch, pull request, or Vibe key.
6. Read any referenced plan or note completely before making cleanup decisions.
7. Create a todo list to track the cleanup tasks.

## Cleanup Philosophy

Cleanup is a safety workflow, not a convenience command. Your job is to leave the repo in a predictable state without deleting unfinished work.

- Prefer preserving work over aggressively removing directories.
- Treat uncommitted or untracked implementation changes as a stop condition.
- Treat the main checkout as a protected baseline that must be inspected before teardown.
- Treat plan completion, main synchronization, worktree removal, and branch deletion as separate decisions.
- Keep local branches unless the user explicitly requests deletion.
- Never delete remote branches as part of this workflow.
- Never use force or remove unrelated worktree metadata.
- Update plan and documentation status before removing the worktree that produced them.
- Explain what you verified, what you cleaned up, and what still needs human attention.

## Cleanup Procedure

### 1. Identify cleanup artifacts

Before changing anything:
- Record which cleanup artifacts are present, absent, ambiguous, blocked, or ready.
- If a Vibe key is available, identify the repository checkout first, then run `vibe status --key <key>` from that repository.
- If `vibe status` fails and no other artifact proves identity, stop and ask for clarification.
- If a target worktree exists, confirm its path, branch, and whether it is a plan worktree or an ad hoc checkout.
- If no target worktree exists and none was explicitly supplied, skip worktree removal and continue with the remaining artifacts.
- If a supplied worktree path does not exist, stop and report the missing explicit artifact.
- If a plan note exists, identify whether it should be marked complete or updated with follow-up status.
- If a pull request or branch exists, identify whether it is only being retained or whether local deletion was explicitly requested.

### 2. Inspect safety conditions

Run these checks before removing a worktree:
- If a target worktree exists, check `git status --short` inside it.
- If a target worktree exists, check for untracked, unusual ignored, locked, detached HEAD, or submodule state that may represent unfinished work.
- If a branch exists, check whether it has commits not present on the fetched base branch.
- If a main checkout exists, inspect it for tracked changes, untracked files that collide with incoming paths, missing upstream, or divergence.
- If a main checkout is clean and can fast-forward, run `git fetch <remote>` and `git pull --ff-only` from the identified main checkout after confirming its branch and upstream.
- If main cannot be safely fast-forwarded, record main synchronization as blocked but continue with unrelated safe cleanup actions.
- Check whether any docs, plans, or research notes still need status updates.

Only stop the whole cleanup for blockers that make the relevant destructive action unsafe. Main-sync-only blockers should be reported while unrelated safe cleanup continues.

If you find uncommitted or unexplained work in an artifact that would be changed or removed, stop that action and report it using this format:

```text
Cleanup blocked

Artifact: [plan, Vibe run, worktree, branch, pull request, remote, or main checkout]
Status: [missing explicit artifact, ambiguous identity, dirty state, unsafe main sync, missing PR evidence, or other blocker]
Reason: [what made cleanup unsafe]

Required next action:
- [what the human should review or decide]
```

Do not remove the worktree until the blocker is resolved or the user explicitly approves a different action.

### 3. Finish housekeeping

For each artifact that is present and safe to update:
- If a plan exists, update its status if the completed goals and verification evidence justify it, then run `planner check <plan.md> --json-errors`.
- Update related documentation or follow-up notes if they are part of the change narrative.
- Summarize any follow-up work that should remain open after cleanup.
- Keep the branch unless the user explicitly requested local deletion.
- Never delete remote branches.
- If local deletion was requested, use `gh pr view <pr> --json state,mergedAt,headRefName,headRefOid,baseRefName,mergeCommit`, `git rev-parse <branch>`, and `git merge-base --is-ancestor <merge-commit> <base-upstream>` to verify one matching merged pull request, a branch tip matching the pull request head OID, and a merge commit reachable from the fetched base branch.
- If no matching pull request exists, the pull request is unmerged, or the branch tip does not match the pull request head OID, keep the branch.
- If local deletion was requested and all checks pass, record branch deletion as pending until no remaining worktree uses that branch.

### 4. Remove the worktree safely

Only after the safety checks pass:
- If an exact worktree exists and is safe, remove it with `git worktree remove <path>` without force.
- If no worktree exists, skip removal.
- Confirm the removed worktree no longer appears in `git worktree list`.
- If local deletion was requested and all checks passed, run `git branch -d <branch>` from another checkout after any worktree using that branch has been removed, then verify the local branch is gone with `git branch --list <branch>`.
- If a main checkout exists, confirm it is still on the expected branch and matches its fetched upstream after any update.
- Do not run global `git worktree prune` as part of targeted cleanup.

## Response Format

When cleanup succeeds, report:

```text
Cleanup complete

Completed:
- [actions completed safely]

Skipped:
- [actions whose artifacts were absent]

Retained:
- [branches, worktrees, files, or plans intentionally kept]

Follow-up:
- [anything still requiring human action, or "None"]
```

When an action is blocked, report the blocker clearly and stop only the unsafe action unless identity is ambiguous.

## Important Guidelines

1. Never remove a worktree that still contains uncommitted or unexplained files.
2. Never assume plan or documentation status is already correct; check it.
3. Prefer explicit verification over inference.
4. Keep the output concise, but include enough detail for a reviewer to understand what changed and what was verified.
