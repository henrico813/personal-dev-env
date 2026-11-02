---
name: pull-request
description: Opens, updates, and merges pull requests safely. Use when a user asks to open, update, or merge a pull request.
metadata:
  pde-workflow: "true"
---

# Pull Requests

Guardrails always apply. Repository instructions may replace the other
steps' defaults.

## Guardrails

1. Push or open/update only when the user asks. Merge only after explicit user
   approval of the shown squash message. Do not rewrite another person's commits.
   Never force push except `--force-with-lease`. Do not add AI attribution.
2. Load `git-messages` before writing a title or description. Review generated
   text for plain English and specific, real claims.
3. Before opening a PR, pushing a review round, or merging, run repository tests
   covering the change. If any fail, stop and report them; never call a failure
   expected or correct in PR text.
4. Before opening a PR for review-ready work, and before Merge step 5, deploy
   the change where it runs and check the changed behavior itself, not only
   that nothing else broke. A deploy that never runs the changed files, such
   as an install on a machine that does not use them, does not count. Send
   the evidence, such as results or screenshots, with the request. If nothing
   can run the change yet, say so in your report and do not send the request.

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
   If it exits 3, tell the user once that a detached runner will
   perform the write when approved. Do not ask the user to confirm.
   Rerun the identical command later only to read its result.
   `already ran <id>, exit N` (followed by the captured `gh` output),
   `declined <id>` (exit 4), and `interrupted <id>` (exit 6) are final.
   Never answer approval requests, run `pde-pr-approve`, or set `PDE_GH_WRITE`.

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
   run `pde-gh-write pr edit NUMBER --body-file BODY` and handle its
   result as in Open step 4.
4. Push the round with `git push --force-with-lease -u <remote> HEAD`.

## Merge

1. Fetch. Build FILE from the PR's current title and description, not a local
   copy: `gh pr view NUMBER --json title,body --jq '.title + "\n\n" + .body' > FILE`.
2. Run the repository tests; stop if any fail. Confirm required checks pass, and
   list exact files with `gh pr diff NUMBER --name-only`.
3. Run `python3 ~/.agents/skills/git-messages/scripts/msg squash FILE > SQUASH`,
   then `python3 ~/.agents/skills/git-messages/scripts/msg lint SQUASH`. If lint
   fails, stop and fix the PR Overview by updating the PR; do not edit SQUASH.
4. Show the exact SQUASH message, file list, check and test results, and the
   live evidence from Guardrail 4.
   The popup or phone approval in step 5 is the user's approval for the
   merge.
5. Run `tail -n +3 SQUASH > BODY`, then run
   `pde-gh-write pr merge NUMBER --squash --subject "$(head -n 1 SQUASH) (#NUMBER)"
   --body-file BODY`. Handle its result as in Open step 4.
6. Confirm the commit on the default branch has that message.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you write, open, update or merge pull
requests, follow [skill-lessons](../skill-lessons/SKILL.md) with the
skill name `pull-request`, and offer once to save the correction as a
lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
