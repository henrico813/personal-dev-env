---
name: skill-fix
description: Writes a correction the user makes into the skill's own
  text and commits it with the skill-fix tool, once the user says yes
  to the exact diff. The tool pushes each fix to one fix pull request
  per PC, so every PC gets it after the merge. Use when the user
  corrects work done under any skill, whether or not its Remembering
  corrections section links here. Do not use it for a skill change
  the user asks for as its own task; skill-development owns that.
metadata:
  scope: personal
---

# Skill Fix

When the user corrects your work under a skill, the correction
belongs in that skill's text, so every PC gets it. The
`skill-fix` tool commits your edit to a fix branch in its own
worktree, never in the user's checkout. It then pushes the branch
and keeps one fix pull request open for this PC. Each new fix joins
that pull request as one more commit.

A fix takes effect only after the user merges the fix pull request
and each PC runs `pde-installer update`. Until then, the correction
holds only in this session.

## Inputs

The linking skill's Remembering corrections section gives:

- the skill's name, for the commands below
- which corrections count, such as "how you plan, write or review
  tests"
- any exception to the lessons in its Lessons section, such as
  "except that the Never list still holds"
- when to offer the fix, such as "once, in your next summary"

A skill with no Remembering corrections section, such as a language
skill, takes every correction to how you did its work. Use its own
name, and offer the fix once, in your next summary.

Here "the user" is the human who asked for the work, not a parent
agent, unless the linking skill names someone else.

## Steps

### 1. Check the correction

1. Write no fix for a rule that holds on this PC only, such as which
   model this PC can run. Offer a setting in
   `~/.config/pde/config.json` when one fits; otherwise tell the user
   the rule holds only in this session.
2. Sort it as "Write a fix into skill text" in the
   [skill-development](../skill-development/SKILL.md) skill says.
   That section says which skill owns the rule, how to word it, and
   when a correction needs no new rule. Its search for an earlier
   fix needs the owner's worktree first: run `skill-fix start` for
   the owner, as step 2 shows.

### 2. Write the fix

1. Get the fix worktree and the file to edit. Replace `<skill>` with
   the name of the skill that owns the rule:

   ```bash
   skill-fix start <skill>
   ```

   The reply's `worktree` is the fix worktree, and `file` is the
   skill's SKILL.md in it. Running it again after step 1 is safe.
   When `edits` is true, the skill's folder holds edits you did not
   make, from another session's fix in progress or one never saved.
   Show them to the user with `git -C <worktree> diff`, or put them in
   your report when a hub started you. Write or apply no fix while
   they are there. When the user says to drop them, run
   `skill-fix start <skill> --reset` and go on.
2. Edit only the files in that skill's folder in the worktree, with
   your usual file tools. A fix may change, add or delete lines in
   SKILL.md or its reference files.
   The fix also adds a correction case under the skill's `evals/`,
   as the `case` row in step 4 says, so a later change that drops
   the rule fails a test.
3. Write the commit message to a new file under `/tmp`. The title is
   `fix(<skill>): <what agents now do>`, at most 50 characters. The
   body has a `Missed:` line on what you did wrong and an `Intent:`
   line on what the fix should change, one sentence each. Leave out
   task details, such as pull request numbers, file paths and
   project names:

   ```text
   fix(herdr-hub): close idle workers yourself

   Missed: the hub asked the user to close a worker that had finished.
   Intent: the hub closes a finished or idle worker's agent itself.
   ```

### 3. Show the diff, then commit

1. Save the fix and clear the worktree before you show it. Every
   session on this PC shares the fix worktree, and `start` refuses
   another skill's fix while yours waits there. Save the diff to a
   new file under `/tmp`, then drop the edits. The first line makes
   the diff show new files, such as a new reference file. When you
   save a fix again, `git apply --3way` has staged its earlier
   edits. Plain `git diff` leaves staged edits out, so diff against
   `HEAD`:

   ```bash
   git -C <worktree> add --intent-to-add .
   git -C <worktree> diff HEAD > <patch file>
   skill-fix start <skill> --reset
   ```

   Then get the privacy review in the `pull-request` skill's
   Guardrail 5, since `commit` can push the fix. Its brief, for a
   fresh agent, lists four things to read: the diff, the commit log,
   the branch name and the PR text. It also has blanks in angle
   brackets, such as `<worktree>`. Fill it in this way:

   - `<worktree>`: the fix worktree.
   - The diff (item 1): give the added lines of the patch file
     instead.
   - The commit log (item 2): give the message file instead.
   - The branch name (item 3): keep it as it is.
   - The PR text (item 4): skip it. The tool writes the fix
     pull request's title and body from the same stock text each
     time, which names no PC.
   - `<BASE>` and `<PR message file>`: leave these blanks empty.
     Only items 1, 2 and 4 use them, and you replace or skip those.

   Skip three steps of the guardrail: its Testing line, its amend
   step and its term scan. The first two change a pull request's
   Testing text and commit, which a fix does not have, and `commit`
   runs the term scan itself.

   To fix a hit in the diff, run `skill-fix start <skill>` and
   `git -C <worktree> apply --3way <patch file>`, edit the fix, and
   save it again as above. Fix a hit
   in the message file in place. Get a new review each time the patch
   or message file changes, in this step or a later one. Show a fix
   only after its review reports zero hits.

2. Offer the fix once, at the time the linking skill names. Show the
   user the message and the diff. To offer several fixes in one
   message, as conclude-session does, write and save each fix in
   turn, then show them all.

   When a hub started you to write the fix, put the message, the diff,
   the patch file's path and the privacy review's `Hits:` line in your
   report and end your turn. The hub reviews them, shows them to the
   user and sends you the answer with `hub send`.
3. Wait for the answer. For each fix the user says yes to, in order,
   run `start`. When its `edits` is true, act as step 2 item 1 says.
   Otherwise apply the fix's diff and commit it:

   ```bash
   skill-fix start <skill>
   git -C <worktree> apply --3way <patch file>
   skill-fix commit <skill> --message-file <file>
   ```

   When `git apply` fails, such as when an earlier fix changed the
   same lines, the folder's edits are yours, since `start` found
   none. Drop them with `skill-fix start <skill> --reset`, write the
   fix again on the new text, save it again as item 1 says, and show
   the new diff.
4. When the user asks for a change, apply the diff as item 3 says,
   edit the fix, save it again as item 1 says, and show the new diff.
   When the user says no, delete the patch file.

### 4. Act on the reply

`commit` prints one JSON object. Act on its `status`. `start` also
prints `refused` with a `rule`, such as `other-files`; act on it as
the table below says:

- `committed`: the fix is safe on its branch. Tell the user once:
  "Fix committed; it reaches every PC after you merge the fix PR."
  When `sync` is not `pushed`, `pr_open` or `pr_waiting`, act on it
  as [references/replies.md](references/replies.md) says.
- `held`: the fix holds an extra term, a word from the file that
  `extra_terms_file` in `~/.config/pde/config.json` names. It stays
  on this PC. Say only: "This fix matched the extra-term list and is
  held on this PC; run `skill-fix held` to see it." Never name or
  guess the term.
- `refused`: `rule` names the fix rule that failed, and `detail`
  names each file and line. Fix the text as the table below says,
  save it again as step 3 item 1 says, and show the user the new
  diff. On a yes, commit it as step 3 item 3 says.
- `error`: the tool failed, and `detail` says why. Tell the user the
  `detail` once. When it starts with `committed`, the fix is
  committed and only the sync failed. Otherwise nothing changed.

| `rule` | What to do |
| --- | --- |
| `no-skill` | Check the skill's name |
| `route` | The skill's `metadata.scope` names no repository set up on this PC. Tell the user the fix was not saved, and give the `detail` |
| `other-files` | A fix changes one skill's folder. When your patch file holds the edits to the files `detail` names, undo them, write the fix again inside the folder, and save and show it as step 3 says. Otherwise they belong to another session's fix: tell the user and stop, and never undo them |
| `width` | Wrap each named line at 72 columns |
| `message` | Make the title pass `msg lint` from the `git-messages` skill, and give the body a `Missed:` and an `Intent:` line |
| `scan` | Remove the private or secret text from each named line; `ci/scan` would refuse to push it |
| `case` | Add a correction case to the skill's `evals/`, as session-review's "Write a correction case" says |

How to write the case is in session-review's
[trials.md](../session-review/references/trials.md#write-a-correction-case).

A `reported` list names findings that did not stop the commit, such
as `repeats`, which names text copied from another skill. It is not
the Repeat kind in skill-development's "Write a fix into skill
text". Mention each finding to the user once.

## Guardrails

- Run `skill-fix commit` only after the user says yes to the diff you
  showed, or to the diff in your report when a hub started you.
- Edit only in the fix worktree. Never edit the installed copy in
  `~/.agents/skills/` or the user's own checkout.
- Never push the fix branch yourself, and never answer the fix pull
  request's approval request. The tool pushes, and `pde-gh-write`
  asks the user.
- A skill tagged `metadata.scope: extra` lives in the extra skills
  repository. When `extra_forge_repo_url` in
  `~/.config/pde/config.json` is set, that repository's fix pull
  requests are on a forge such as Gitea, not GitHub. The tool then
  opens the pull request there without asking, since the user chose
  this in the setting. The user merges it in the forge's web page.
- Never read or use the forge API token in the file that
  `extra_forge_token_file` names. The token can also merge pull
  requests, and only the user merges.
- Never save a new lesson to a Lessons section with the older
  `skill-lessons add` command.

## When a tool is missing

- If `skill-fix` is not installed, apply the correction in this
  session only, and say once that it was not saved.
- If the `skill-development` skill is not installed, as on a client
  PC, read `ai/skills/skill-development/SKILL.md` in the fix
  worktree of a personal skill.
- In a Vibe worker, whose skills folder is read-only, write no fix.

## Final checks

- The user knows whether the fix was committed or not saved.
- The user saw the exact diff before `commit` ran.
- A privacy review of the saved fix reported zero hits before the
  user saw it.
- The fix changes one skill's folder and passes the Final checks of
  skill-development.
- The message has a `Missed:` and an `Intent:` line, with no task
  details.
- A held fix's message to the user names no term.

## References

- [references/replies.md](references/replies.md): read when `sync` is
  not `pushed`, `pr_open` or `pr_waiting`, or when the user asks
  about held fixes or the fix pull request.
- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

When the user corrects how you check, write, show or commit a fix,
follow this skill with the skill name `skill-fix`, and offer once to
fix the skill's text. A correction to how a fix is worded or where
its rule goes belongs to `skill-development` instead.
