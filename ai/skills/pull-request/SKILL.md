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
4. Show the exact SQUASH message, file list, and check and test results.
   The popup or phone approval in step 5 is the user's approval for the
   merge.
5. Run `tail -n +3 SQUASH > BODY`, then run
   `pde-gh-write pr merge NUMBER --squash --subject "$(head -n 1 SQUASH) (#NUMBER)"
   --body-file BODY`. Handle its result as in Open step 4.
6. Confirm the commit on the default branch has that message.

## Remembering corrections

Here "the user" is the human who asked for the work, not a parent
agent. Follow every lesson in the Lessons section below. Each is a
correction the user made, so a lesson wins when it disagrees with the
text above. If the section is empty and `skill-lessons` is installed,
run `skill-lessons render` once, since an install can reset this file,
and read the section again.

When the user corrects how you write, open, update or merge pull
requests, still save it to memory as your instructions say. If
`skill-lessons` is installed, also offer once to save it here as a
lesson. When the user says "remember", save it at once and show the one
line it prints:

    jq -n --rawfile text /dev/stdin \
      '{skill: "pull-request", text: ($text | rtrimstr("\n"))}' \
      <<'EOF' | skill-lessons add -
    <rule>
    EOF

The rule goes on its own line, so apostrophes, quotes, backticks and
`$` are saved as written. If the user corrects a lesson again, add
`repeat_of: "L<n>"` inside the braces, with the ID from the end of that
lesson. If `skill-lessons` is not installed, say so once and save no
lesson. If the skills folder is read-only, as in a Vibe worker, follow
the lessons shown and save or render nothing. Lessons become rules in
this text only through a pull request the user asks for.

Offer a lesson only for a rule an agent would apply correctly in
another repository or task. Write it without facts true only of this
task, such as its PR numbers, option labels, models on trial or
one-time slips. If no such rule is left, offer no lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
