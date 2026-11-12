---
name: conclude-session
description: Closes out the current session after one reply from the
  user. Runs session-review, adds a handoff, a status update and a
  cleanup to its pick list, then saves the picked lessons, opens a
  pull request with the picked skill and rules-file edits, and runs
  the picked closing steps. Use when the user types conclude-session
  or asks to close out or wrap up this session.
metadata:
  pde-workflow: "true"
---

# Conclude Session

The user ends a session with a run of prompts: a session review,
picks from it, lessons to save, a pull request, then a status update
or a handoff. This skill runs those steps and stops once, for the
user's pick.

## Inputs

- This session. Review no other session.
- This conversation, for the user's corrections and the work left.

## Steps

### 1. Review

Load [session-review](../session-review/SKILL.md) and follow its
steps 1 to 5 for this session. Write the review yourself, in this
session, because a subagent would have to read the whole
conversation again. Session-review's fresh check in its step 5 still
runs, since it reads no messages.

When the script's `compactions` line is above 0, the corrections
made before the last compaction are no longer in your context. Print
only the user's messages from before it, each cut to 200 characters:

    python3 - <<'EOF'
    import json, os, sqlite3
    from pathlib import Path
    db = Path.home() / ".local/share/opencode/opencode.db"
    rows = sqlite3.connect(f"file:{db}?mode=ro", uri=True).execute(
        "select type, seq, data from session_message"
        " where session_id = ? order by seq",
        (os.environ["OPENCODE_SESSION_ID"],),
    ).fetchall()
    last = max(
        (seq for kind, seq, data in rows if kind == "compaction"
         and json.loads(data).get("status") == "completed"),
        default=0,
    )
    for kind, seq, data in rows:
        if kind == "user" and seq < last:
            text = json.loads(data)["text"].replace("\n", " ")
            print(seq, text[:200])
    EOF

Name this query in the review, as session-review asks for any query
of your own. Never print whole messages.

### 2. Ask once

Send session-review's step 6 message with three changes:

- Number every item in one run, proposals first, then lessons, so
  each number names one item.
- Before its last line, add a Closing steps list, numbered after the
  lessons, with each step that applies:
  - Handoff: hand the work that is left to a new session. Offer it
    when work is left, including work planned but not started.
  - Status update: a recap a new reader follows.
  - Cleanup: remove a merged plan's worktree and branch. Offer it
    only when a plan's pull request merged in this session.
- In its last line, say that nothing has changed yet, and accept
  "none".

Then wait, and change nothing until the user replies. When the reply
is a question, such as "what is 2?", answer it and wait again. A
reply of "none" ends the skill with nothing changed.

### 3. Act on the picks

Do the picked items in this order, so the last message can name
everything that changed:

1. Lessons. Save each picked lesson with the command in step 4 of
   [skill-lessons](../skill-lessons/SKILL.md). The pick counts as the
   user saying "remember". Save a picked memory as your instructions
   say.
2. Skill and rules-file edits. The source of a shared skill is in
   the personal-dev-env checkout, under `ai/skills/<name>/`, not the
   installed copy. Ask the user for the checkout's path when you
   cannot find it. Create one branch and worktree in it from the
   default branch, as your instructions say. Edit only there. Make
   each skill edit as "Change a skill" in the `skill-development`
   skill says, including its two reviews. Commit with the
   `git-messages` skill. Step 6 of "Change a skill" says when to open
   the pull request. Open it with the
   [pull-request](../pull-request/SKILL.md) skill. A rules file in
   another repository gets its own branch and pull request there.
   When another session owns a rules file, send it the change
   instead, as session-review says.
3. Cleanup, with the [cleanup-plan](../cleanup-plan/SKILL.md) skill.
4. Handoff, with the [herdr-handoff](../herdr-handoff/SKILL.md)
   skill. Name the pull request and the saved lessons in the handoff
   file.

When a pull request still waits for the user's approval, name its
branch and say that the request is waiting. Never write a pull
request number or link you have not seen.

### 4. Report

End with one message by the
[status-updates](../status-updates/SKILL.md) skill. When the user
picked a status update, this message is it. Otherwise keep it to the
lessons saved, with the line each save printed, each pull request,
the handoff's pane or file, and each pick you could not do, with
why.

## Output format

Step 2's message, for a session with one proposal and one lesson:

    Session review of Tally P5 build, saved to
    ~/.local/state/pde/session-reviews/20261105-tally-p5.md.

    11.2M tokens read, peak context about 171,400, 120 steps.

    Proposals
    1. Print only failing tests in roadmap-run's test step, in a
       skill PR. Saves about 1.4M tokens (estimate: 4,000 tokens
       read again by 90 steps, 4 times).

    Lessons to save, pick by number
    2. Print test failures, never whole test logs -> roadmap-run

    Local models: a script could run the 5 test runs.

    Closing steps
    3. Hand the P6 build to a new session.
    4. Status update.

    Nothing has changed yet. Reply with the numbers to apply, or
    none.

## Guardrails

- Never merge the pull request, and never answer its approval
  request. The user approves it.
- Change only what the user picked.
- Offer each item once per session. When the user turns down a whole
  kind of proposal, offer once to save a lesson that starts "Do not
  propose" under the skill name `session-review`, not
  `conclude-session`, as session-review says.
- Edit skill and rules files only on the branches from step 3, never
  on the default branch, in the installed copy under
  `~/.agents/skills`, or on the session's feature branch.
- Read no other session, and no whole message from the database.
- Never turn saved lessons into skill text. That is a separate job
  the user starts, and it follows the `skill-development` skill.

## When a tool is missing

- The script fails, or `skill-lessons` is not installed: do what
  session-review's When a tool is missing section says.
- The compaction query fails: say in the review that it leaves out
  corrections from before the compaction.
- The pull request cannot be opened: give the user the branch and
  worktree path, and leave both in place.

## Final checks

- One message asked for picks, and nothing changed before the reply.
- Every change matches a picked number.
- Skill and rules-file edits sit only on close-out branches, one
  pull request per repository, none merged by you.
- The last message names the saved lessons, each pull request or
  waiting request, and each pick not done.

## References

- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you close out a session, follow
[skill-lessons](../skill-lessons/SKILL.md) with the skill name
`conclude-session`, and offer once to save the correction as a lesson.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
