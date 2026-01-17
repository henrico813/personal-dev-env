---
description: Create a new git worktree that tracks the remote default branch
argument-hint: <name> (e.g., auth-feature, bugfix-login)
allowed-tools: Bash(git:*)
model: haiku
---

# Git Worktree Command

## Context

Current repository info: !`git remote get-url origin 2>/dev/null || echo "No remote origin found"`

Current branch: !`git branch --show-current`

Existing worktrees: !`git worktree list`

Default remote branch: !`git remote show origin 2>/dev/null | grep "HEAD branch" | cut -d: -f2 | xargs || git branch -r | grep -E 'origin/(main|master)' | head -1 | sed 's/.*origin\///'`

## Your task

Create a new git worktree with the provided name that tracks the remote default branch.

**Arguments:** $ARGUMENTS

**Behavior:**
1. Validate that a name was provided. If not, ask the user for a name.
2. Determine the default remote branch (origin/main or origin/master)
3. Create a new branch with the provided name
4. Create the worktree at `../name` (sibling to current repo)
5. Set the new local branch to track the remote default branch as upstream

**Process:**
1. Fetch latest changes from remote: `git fetch origin`
2. Determine the default remote branch:
   - First, try `git remote show origin | grep "HEAD branch"` to find the repository's default branch name
   - Verify the branch exists in remote with `git branch -r`
   - If the default branch doesn't exist remotely or cannot be determined, check for origin/main, then origin/master
   - If neither exists, fail with a clear error message
3. Create the worktree tracking the remote branch:
   - Use: `git worktree add --track -b <name> ../<name> origin/branch-name`
   - This creates the branch, sets up tracking, and creates the worktree directory
4. Display success message with:
   - The new branch name
   - The absolute path to the worktree
   - The upstream tracking branch
   - How to switch to the worktree: `cd ../<name>`

**Examples:**
- `/worktree auth-feature` - Creates branch `auth-feature` at `../auth-feature` tracking origin/main
- `/worktree bugfix-login` - Creates branch `bugfix-login` at `../bugfix-login` tracking origin/main
