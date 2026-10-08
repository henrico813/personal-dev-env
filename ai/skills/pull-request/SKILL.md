---
name: pull-request
description: Opens, updates, and merges pull requests safely. Use when a user asks to open, update, or merge a pull request.
metadata:
  pde-workflow: "true"
---

# Pull Requests

Use repository instructions first. Use these defaults otherwise.

## Guardrails

1. Push or open/update only when the user asks. Merge only after explicit user
   approval of the shown squash message. Do not rewrite another person's commits.
   Never force push except `--force-with-lease`. Do not add AI attribution.
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
   Run `pde-gh-write pr create --title "$(head -n 1 FILE)" --body-file BODY`.
   If it exits 3, tell the user to approve the popup or run `pde-pr-approve` in
   a terminal, wait for confirmation, then rerun the identical command. If it
   exits 4, report that the user declined. Never run `pde-pr-approve` yourself
   or set `PDE_GH_WRITE`.

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
   Run `pde-gh-write pr edit NUMBER --body-file BODY`. If it exits 3, tell the
   user to approve the popup or run `pde-pr-approve` in a terminal, wait for
   confirmation, then rerun the identical command. If it exits 4, report that
   the user declined. Never run `pde-pr-approve` yourself or set `PDE_GH_WRITE`.
4. Push the round with `git push --force-with-lease -u <remote> HEAD`.

## Merge

1. Fetch. Build FILE from the PR's current title and description, not a local
   copy: `gh pr view NUMBER --json title,body --jq '.title + "\n\n" + .body' > FILE`.
2. Run the repository tests; stop if any fail. Confirm required checks pass, and
   list exact files with `gh pr diff NUMBER --name-only`.
3. Run `python3 ~/.agents/skills/git-messages/scripts/msg squash FILE > SQUASH`,
   then `python3 ~/.agents/skills/git-messages/scripts/msg lint SQUASH`. If lint
   fails, stop and fix the PR Overview by updating the PR; do not edit SQUASH.
4. Show the exact SQUASH message, file list, and check and test results. Ask for
   approval. The popup approval below is the user's approval for the merge.
5. Run `tail -n +3 SQUASH > BODY`, then run
   `pde-gh-write pr merge NUMBER --squash --subject "$(head -n 1 SQUASH) (#NUMBER)"
   --body-file BODY`. If it exits 3, tell the user to approve the popup or run
   `pde-pr-approve` in a terminal, wait for confirmation, then rerun the
   identical command. If it exits 4, report that the user declined. Never run
   `pde-pr-approve` yourself or set `PDE_GH_WRITE`.
6. Confirm the commit on the default branch has that message.
