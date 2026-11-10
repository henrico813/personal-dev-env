---
name: comment-review
description: Saves code comments as a review that the user answers one
  comment per screen with glimmer; the answers are then
  written into the code with no AI involved. Use when you want to put
  code comments in front of the user for review, or a task's last step
  is the user's review of the comments.
---

# Comment Review

The user reviews comments one per screen, on a phone or in a
terminal, with `glimmer`. You send a Feedback Request
List, with your Agent Suggestion to keep, cut or reword each comment;
the user answers each Feedback Request; after the last answer the
tool writes the answers into the code and approves the answered
comments, so `check-comments` stops later agents from changing them.

If your harness lists the MCP tools `check_comments`,
`make_feedback_request_list`, `send_agent_suggestions` and
`show_status`, perhaps with a prefix, use them in place of the
commands below. Each takes `dir`, the checkout's absolute path.
`check_comments` and `make_feedback_request_list` also take `base`,
`make_feedback_request_list` takes `change_requests`, and
`send_agent_suggestions` takes `list`, the list itself. They give the
same JSON as the commands.

The tool reads Go, Python and Rust comments. Show the comments of
other languages to the user in the pull request or the chat instead.

## Steps

Run each command in the task's worktree. Each prints JSON: the
review's `status`, the `result`, an `error` with a `code` and a
`message` when it refused, and `next`, the step to take.

1. Never edit a comment the user approved, even when a plan says
   to: ask to change it in step 3. Optional: before step 2, have a
   fresh reviewer, such as a new agent session given only the changed
   code, shorten or cut the comments you wrote the way the user's
   earlier notes asked.
2. Run the comment check of the `code-documentation` skill until it
   reports no failures. `<task start>` is the commit your task
   started from:

   ```bash
   glimmer check-comments --base <task start>
   ```

3. Build the Feedback Request List. This saves nothing:

   ```bash
   glimmer make-feedback-request-list --base <task start>
   ```

   `result.list` holds a Feedback Request for each comment the branch
   added or changed, and each comment that goes against an approval
   or cut. Its `reason` is `new`, `was_cut` (the user cut it before),
   `differs_from_approved` (the user approved other text) or
   `agent_wants_change`. If `next` is
   `finish_without_feedback_requests`, tell the user there is nothing
   to review and stop.
   To change a comment the user approved, write an input file outside
   the repository, such as in a folder from `mktemp -d`, and pass it
   with `--input FILE`. Each `file` and `name` is as the comment's
   entry in the approvals file shows it. Read that file only, at
   `$(git rev-parse --git-common-dir)/pde/code-comments.json`:

   ```json
   {"change_requests": [{"file": "store.go", "name": "LoadAnswers"}]}
   ```

   The list then holds an `agent_wants_change` request for it. While
   a list waits for the user, this step refuses with
   `waiting_for_user_feedback`; list the change in your final message
   instead.
4. Save `result.list` to a file outside the repository, and give each
   request a `suggestion`, changing nothing else:

   ```json
   {"kind": "keep", "reasoning": "Says what it returns.", "also_at": []}
   ```

   `kind` is `"keep"`, `"cut"` or
   `{"reword": {"text": "Returns the total, in cents."}}`, the new
   wording as plain sentences without `//` or `#`; the tool wraps and
   indents it. A request whose `can_reword` is false takes only keep
   or cut. `reasoning` is one or two sentences the user sees as
   `why:`. `also_at` lists other places that say the same thing, as
   `file:line`.
5. Send the list to the user:

   ```bash
   glimmer send-agent-suggestions <list file>
   ```

   When it refuses with `list_changed`, the code changed: run step 3
   again with the same input file, then step 4. When it refuses a
   suggestion, fix what its `message` names and send again. Follow
   `next` on any other refusal, such as `check_failed` or
   `waiting_for_user_feedback`. When `next` is `tell_user_and_wait`,
   tell the user: "A comment review is ready: press Ctrl+b e in Herdr,
   or Ctrl+b E for a full tab, or run
   `glimmer open <branch>`." Do not run `open` yourself: it
   takes over the terminal it runs in. Do not edit the listed comments
   before the last answer: the write refuses a comment that changed.
6. When the user says there are User Notes, run
   `glimmer show-status`; `result.waiting.user_notes`
   holds each note by request ID. Run step 3 again: it returns the
   noted requests with their IDs. Answer each note in that request's
   Agent Suggestion, without editing the comment in the code, then
   step 5 once. Only those requests need a new answer; every other
   answer stays.
7. Never commit `.code-comments.json` or anything in `.git/pde/`.
   Never edit the review's saved files, `review.json` in the
   worktree's Git folder or `pde/code-comments.json` in the clone's
   shared Git folder: change a review only with these commands, and
   leave the answers and notes to the user.
8. Run `glimmer show-status` at any time to see what the
   review waits on and the step to take next.
9. When the user tells you to drop the review, run
   `glimmer dismiss` in the worktree, so Ctrl+b e stops
   showing it; `glimmer restore` brings it back. The user
   can also press `x` in Ctrl+b e's list. A `dismissed` status means
   send nothing more unless the user asks for a new review.

## When the tool is missing

If `glimmer` is not on `PATH`, tell the user that the
comment review tool is not installed, and show the comments to the
user in the pull request or the chat instead.
