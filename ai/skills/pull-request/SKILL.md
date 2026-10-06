---
name: pull-request
description: Opens, updates, and merges pull requests safely. Use when a user asks to open, update, or merge a pull request.
metadata:
  pde-workflow: "true"
---

# Pull Requests

Use repository instructions first. Use these defaults otherwise.

## Guardrails

1. Push or open/update only when the user asks. Merge only on an explicit
   request. Do not rewrite another person's commits. Never force push except
   `--force-with-lease`. Do not add AI attribution.
2. Load `git-messages` before writing a title or description. Review generated
   text for plain English and specific, real claims.
3. Before opening a PR, pushing a review round, or merging, run repository tests
   covering the change. If any fail, stop and report them; never call a failure
   expected or correct in PR text.

## Discover

1. Run `git remote -v`. Find the default branch with
   `git symbolic-ref --short refs/remotes/<remote>/HEAD`; if that fails, use
   `gh repo view --json defaultBranchRef --jq .defaultBranchRef.name`. Read the
   PR template and repository instructions.
2. Default to squash. Use another method only when docs or user instructions
   require it, the forge disallows squash, or the default branch consistently
   uses another method. State the signal.

## Open

1. Write FILE as the title, a blank line, and the three PR sections. Add one
   Changes bullet per change and one real Testing block.
2. Run `python3 ~/.agents/skills/git-messages/scripts/msg fmt FILE`, then
   `python3 ~/.agents/skills/git-messages/scripts/msg lint --pr FILE`. Fix only
   reported lines, up to three tries. Repositories may keep their own copy.
3. Fetch and squash existing branch work into one phase commit:
   `git fetch <remote>`
   `git reset --soft "$(git merge-base HEAD <remote>/<default>)"`
   `git commit -F FILE`
4. Push the phase with `git push --force-with-lease -u <remote> HEAD`.
   Create the description without the title:
   `tail -n +3 FILE > BODY`
   Then run `gh pr create --title "$(head -n 1 FILE)" --body-file BODY`.

## Update

1. Fetch and inspect the PR before editing. Keep each review round as one
   commit. Squash fixups within the current round before pushing.
2. Write the round's commit message to COMMIT as a title, blank line, and
   short body without PR sections. Run `python3 ~/.agents/skills/git-messages/scripts/msg fmt COMMIT`, then
   `python3 ~/.agents/skills/git-messages/scripts/msg lint COMMIT`.
   Commit it with `git commit -F COMMIT`. Keep FILE as the PR message.
3. Never rewrite a reviewed phase. Append a Changes bullet per change and
   append or update Testing blocks with checks actually run. Run fmt and lint
   --pr on FILE, then `tail -n +3 FILE > BODY` and
   `gh pr edit NUMBER --body-file BODY`.
4. Push the round with `git push --force-with-lease -u <remote> HEAD`.

## Merge

1. Fetch and confirm the PR head. Run `gh pr diff NUMBER --name-only`, inspect
   the exact file list, and confirm required checks pass.
2. Re-read the final description against the final diff, then lint FILE.
   Create `BODY` with `tail -n +3 FILE > BODY`.
3. For squash, pass the reviewed message explicitly:
   `gh pr merge NUMBER --squash --subject "$(head -n 1 FILE) (#NUMBER)" --body-file BODY`.
4. Confirm the resulting commit is on the default branch. If another merge
   method, conflicts, changed scope, missing `gh`, or another unusual state
   appears, stop and give the user exact commands instead.
