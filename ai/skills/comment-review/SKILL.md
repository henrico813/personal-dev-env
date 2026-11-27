---
name: comment-review
description: Saves code comments as a review that the user answers one
  comment per screen with glimmer; the answers are then
  written into the code with no AI involved. Holds the rules for
  comments the user approved. Use when you want to put code comments
  in front of the user for review, when a task's last step is the
  user's review of the comments, or before you change or remove a
  comment the user approved. Not for writing or checking comments,
  which the code-documentation skill covers.
metadata:
  scope: personal
---

# Comment Review

The user reviews comments one per screen, on a phone or in a
terminal, with `glimmer`. You send a Feedback Request
List, with your Agent Suggestion to keep, cut or reword each comment;
the user answers each Feedback Request; after the last answer the
tool writes the answers into the code and approves the answered
comments, so `check-comments` stops later agents from changing them.

`glimmer help COMMAND` says what each command below prints and what
to do about each refusal; read it before you first run a command. If
your harness lists glimmer's MCP tools, perhaps with a prefix, use
them in place of the commands; `glimmer help mcp` says what each
takes.

The tool reads Go, Python and Rust comments. Show the comments of
other languages to the user in the pull request instead. Never list
comments in the chat for the user to answer.

## Approved comments

The user owns each decision they made in a review, to approve or
cut a comment. glimmer saves those decisions in the approvals file,
`$(git rev-parse --git-common-dir)/pde/code-comments.json`, which
every worktree of the clone shares.

- Never edit a comment the user approved, even when a plan says to
  or the text seems false. Ask to change it with a change request,
  as step 3 says, or report it in your final message.
- Never edit the approvals file. Read it only for the `file` and
  `name` of a comment you ask to change.

## Steps

Run each command in the task's worktree. `<task start>` is the
commit your task started from.

1. Before step 2, start a fresh reviewer, such as a new agent session
   given only the changed code. It shortens or cuts the comments you
   wrote, as the user's notes in past comment reviews asked. When you
   cannot start one, say so in your report.
2. Run the comment check as the `code-documentation` skill says,
   until it reports no failures.
3. Build the Feedback Request List. This saves nothing:

   ```bash
   glimmer make-feedback-request-list --base <task start>
   ```

   List each comment that step 2 reports with the note `existing` in
   your final message, since the list leaves it out. If `next` is
   `finish_without_feedback_requests`, tell the user there is nothing
   to review and stop.
   To change a comment the user approved, write a change request file
   in the form `glimmer help make-feedback-request-list` shows,
   outside the repository, such as in a folder from `mktemp -d`, and
   pass it with `--input FILE`. While a list waits for the user, list
   the change in your final message instead.
4. Save `result.list` to a file outside the repository, and give each
   request a `suggestion`, changing nothing else. Write each
   `reasoning` as one or two sentences.
5. Send the list to the user:

   ```bash
   glimmer send-agent-suggestions <list file>
   ```

   On a refusal, do what the command's help says. When `next` is
   `tell_user_and_wait`, tell the user: "A comment review is ready:
   press Ctrl+b e in Herdr, or Ctrl+b E for a full tab, or run
   `glimmer open <branch>`." Do not run `open` yourself: it
   takes over the terminal it runs in. Do not edit the listed comments
   before the last answer: the write refuses a comment that changed.
6. When the user says there are User Notes, run
   `glimmer show-status` to read them. Run step 3 again: it returns
   the noted requests with their IDs. Answer each note in that
   request's Agent Suggestion, without editing the comment in the
   code, then step 5 once. Only those requests need a new answer;
   every other answer stays.
7. Never commit `.code-comments.json` or anything in `.git/pde/`.
   Never edit the review's saved file, `review.json` in the
   worktree's Git folder: change a review only with these commands,
   and leave the answers and notes to the user.
8. Run `glimmer show-status` at any time to see what the
   review waits on and the step to take next.
9. When the user tells you to drop the review, run
   `glimmer dismiss` in the worktree, so Ctrl+b e stops
   showing it; `glimmer restore` brings it back. The user
   can also press `x` in Ctrl+b e's list. A `dismissed` status means
   send nothing more unless the user asks for a new review. Ctrl+b e
   dismisses a review itself once its pull request is merged or
   closed, or its worktree has another branch checked out, so do not
   dismiss those by hand.

## When the tool is missing

If `glimmer` is not on `PATH`, tell the user that the
comment review tool is not installed, and show the comments to the
user in the pull request instead.

## References

- [references/evaluation.md](references/evaluation.md): read only
  when testing or revising this skill.

## Remembering corrections

Follow every lesson in the Lessons section below. Each is a correction
the user made, so a lesson wins when it disagrees with the text above.
When the user corrects how you run a comment review or treat a comment
they approved, follow [skill-fix](../skill-fix/SKILL.md) with the
skill name `comment-review`, and offer once to fix the skill's text.

## Lessons
<!-- skill-lessons: filled from the local store; do not edit -->
