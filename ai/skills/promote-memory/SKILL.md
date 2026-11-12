---
name: promote-memory
description: Use when the user asks to promote a saved behavioral learning or memory preference into source-controlled guidance.
---

# Promote Memory

Turn one explicitly requested learning into the smallest reviewed source
change. A memory write never authorizes this work on its own.

## Scope

- Default scope is global. Work in a `personal-dev-env` checkout and open a
  pull request against `henrico813/personal-dev-env`, regardless of the
  current repository.
- Edit the current repository only when the user explicitly asks for
  repository-local guidance.
- Create a branch and worktree from `main` with
  `wt switch --create <branch> --no-cd --format=json`, and work in the
  `path` it prints; never commit directly to `main`.

## Workflow

1. Restate the learning as one concise, future-facing rule.
2. Place the rule, write the change and run its checks as steps 1
   to 5 of "Change a skill" in the `skill-development` skill say.
3. Run `(cd pde-installer && go test ./... && go vet ./...)` and
   `git diff --check` before the reviews in step 4 below. That way,
   any fix those checks need is already in the change the reviewers
   read.
4. Get the reviews that step 6 of "Change a skill" describes.
5. Before committing, verify that `git config user.email` ends
   in `@users.noreply.github.com`. Stop and report a mismatch.
6. Load `git-messages` and commit. When step 6 of "Change a skill"
   allows a pull request, open one with the `pull-request` skill.
