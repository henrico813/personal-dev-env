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
2. Load `git-messages` before writing a title or description. Load and
   follow [plain-direct-english](../plain-direct-english/SKILL.md) for
   that text too, and review it for specific, real claims.
3. Before opening a PR, pushing a review round, or merging, run repository tests
   covering the change. If any fail, stop and report them; never call a failure
   expected or correct in PR text.
4. Before opening a PR, and before Merge step 5, deploy the change
   where it runs and try the changed behavior itself, not only that
   nothing else broke. A deploy that never runs the changed files,
   such as an install on a machine that does not use them, does not
   count. For a skill or prompt, deploy it by installing it, and have
   a fresh agent follow it on one real task. Put a one-line result in
   Testing, and give the rest, such as screenshots, in your message to
   the user. If the change only runs after its own merge, such as a
   job that runs only on the default branch, or never runs, such as
   docs, say which in Testing and how you checked it. If the change
   runs somewhere you cannot reach yet, including a stacked PR that
   needs its parent merged, tell the user and do not send the
   request. To undo a test install, install again from the default
   branch.

## Discover

1. Run `git remote -v`. Find the default branch with
   `git symbolic-ref --short refs/remotes/<remote>/HEAD`; if that fails, use
   `gh repo view --json defaultBranchRef --jq .defaultBranchRef.name`. Read the
   PR template and repository instructions.
2. Default to squash. Use another method only when docs or user instructions
   require it, the forge disallows squash, or the default branch consistently
   uses another method. State the signal.

## Open

1. Write FILE as the title, a blank line, and the three PR sections,
   shaped for a phone reader as `git-messages` item 5 says. Paste the
   whole draft into the chat for the user to review, rather than a
   path to the file.
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
   Before the first request goes out, tell the user who approves each
   request and when.
   Run `pde-gh-write pr create --title "$(head -n 1 FILE)" --body-file BODY`.
   If it exits 3, tell the user once that a detached runner will
   perform the write when approved. Do not ask the user to confirm.
   Rerun the identical command later only to read its result.
   `already ran <id>, exit N` (followed by the captured `gh` output),
   `declined <id>` (exit 4), and `interrupted <id>` (exit 6) are final.
   After `already ran` with N not 0, fix the cause, then ask again by
   putting `--retry` before `pr` in the same command.
   Never answer approval requests, run `pde-pr-approve`, or set `PDE_GH_WRITE`.

## Update

1. Fetch and inspect the PR before editing. Keep each review round as one
   commit. Squash fixups within the current round before pushing.
2. Write the round's commit message to COMMIT as a title, blank line, and
   short body without PR sections. Run `python3 ~/.agents/skills/git-messages/scripts/msg fmt COMMIT`, then
   `python3 ~/.agents/skills/git-messages/scripts/msg lint COMMIT`.
   Commit it with `git commit -F COMMIT`. Keep FILE as the PR message.
3. Never rewrite a reviewed phase. Fold each round's fixes into the
   Changes lines they belong to, and update Testing with checks
   actually run, as `git-messages` item 5 says. Run fmt and lint
   --pr on FILE, then `tail -n +3 FILE > BODY` and
   run `pde-gh-write pr edit NUMBER --body-file BODY` and handle its
   result as in Open step 4.
4. Push the round with `git push --force-with-lease -u <remote> HEAD`.

## Merge

For a pull request the user reviewed, the `herdr-hub` skill's merge
worker script runs these steps. It sends step 5's request itself, and
the approval popup shows the squash message. For the script, the try
recorded in Testing when the pull request was opened or last updated
counts as Guardrail 4. The script merges the head it tests. When the
branch is behind, that head is a new commit that merges the base in.
Without such a try in Testing, a cloud session started by
`herdr-handoff` follows Merge from step 1 instead, on the cheapest
cloud model that does it well.

1. Run the merge checks from the repository, with each test command
   as one argument:

   ```sh
   python3 ~/.agents/skills/pull-request/scripts/merge_prep.py \
     NUMBER '<test command>'
   ```

   It fetches and works from the PR's current title and description,
   not a local copy. In a repository with several remotes, add
   `--remote <remote>`. It prints at most 20 lines. In later
   commands, use the folder on its `out:` line as `$O`.
2. It runs the tests on the PR merged into its base, in a scratch
   clone, so no checkout changes. It also lists the changed files and
   counts GitHub's checks. When its last line starts with
   `result: not ready`, that line names each problem. On an `error:`
   line, stop and report it. Stop on any problem but `squash
   message`, which step 3 fixes. Full test output is in
   `$O/tests/test<N>.log`.
3. It also builds the squash message in `$O/squash.txt` and lints it.
   If that fails, fix the PR title or Overview by updating the PR, and
   run step 1 again; do not edit `$O/squash.txt`.
4. Show the exact message in `$O/squash.txt`, the files in
   `$O/files.txt`, the check and test results, and the result from
   Guardrail 4.
   The popup or phone approval in step 5 is the user's approval for the
   merge.
5. Run `pde-gh-write pr merge NUMBER --squash --subject
   "$(head -n 1 "$O/squash.txt") (#NUMBER)" --body-file "$O/body.txt"`.
   Handle its result as in Open step 4.
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
