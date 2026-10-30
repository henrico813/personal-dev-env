---
name: comment-review
description: Saves code comments as a review that the user answers one
  comment per screen with pde-comment-review; the answers are then
  written into the code with no AI involved. Use when you want to put
  code comments in front of the user for review, or a task's last step
  is the user's review of the comments.
---

# Comment Review

The user reviews comments one per screen, on a phone or in a
terminal, with `pde-comment-review`. You save your proposals as a
review; the user answers each one; after the last answer the tool
writes the answers into the code and approves the answered comments,
so the comment checker stops later agents from changing them.

If your harness lists the MCP tools `review_status`, `review_draft`,
`review_set` and `review_submit`, perhaps with a prefix, use them in
place of `status`, `draft`, `set` and `submit`. Each takes `dir`, the
checkout's absolute path, and `review_set` also takes `id`, `answer`
and `text`. They reply with the same JSON as the commands, naming
tools as next steps.

The tool supports Go comments today. Until it supports a language,
show that language's comments to the user in the pull request or
the chat instead.

## Steps

`status`, `draft`, `set` and `submit` each print JSON: the review's
`state`, the `items` or `notes` to act on, any `problems`, and
`next`, the step to take.

1. Never edit a comment the user approved, even when a plan says
   to: propose your change in step 8. Optional: before `draft`, have
   a fresh reviewer, such as a new agent session given only the
   changed code, shorten or cut the comments you wrote the way the
   user's earlier notes asked.
2. On the task's branch, in the repository, run:

   ```bash
   pde-comment-review draft
   ```

   It finds every Go comment the user must answer: comments the
   branch added or changed, approved comments that were edited or
   renamed, and approved comments whose code was removed, which the
   user can only drop (`code_removed`). It saves them as the draft
   of a new review named after the branch. If `state` is `done`,
   tell the user there is nothing to review.
   When it refuses, it names each problem and saves nothing; fix them
   and run it again. If a round waits for the user, run
   `pde-comment-review status` and follow `next`.
3. Optional: change an item's proposal to `keep`, `cut`, or `reword`
   with the new wording as plain sentences, without `//`; the tool
   wraps and indents it. Leave `code_removed` items as `cut`:

   ```bash
   pde-comment-review set 2 reword "Returns the total, in cents."
   ```

4. Send the round to the user:

   ```bash
   pde-comment-review submit
   ```

   When it names problems, it saved nothing: fix them and follow
   `next`. If you changed code to fix them, run `draft` again and
   repeat your `set` changes. If a comment has the same wording as
   another comment in its file, or as the approved text of one you
   edited, change an unapproved copy's wording in the code. Then tell
   the user what `next` says: "Review shop is ready: press Ctrl+b e in
   Herdr, or Ctrl+b E for a full tab, or run
   `pde-comment-review open shop`." Do not run `open`
   yourself: it takes over the terminal it runs in. Do not edit the
   listed comments before the last answer: the write refuses an
   edited one or puts the user's answer over your edit.
5. When the user says there are notes, run `pde-comment-review
   status`. Answer each note with `set` and the item's `id`, without
   editing the comment in the code, then `submit` once. Only those
   items need a new answer; every other answer stays. If the user
   asks for removed code back, put it back with its approved comment,
   then `set` the item to `keep`.
6. Never commit `.code-comments.json`, the tool's file at the repo root.
7. Never edit a review's saved files in
   `~/.local/state/pde/comment-review/`: change a review only with
   these commands and `add`, and leave the answers and notes to the
   user.
8. To change an approved comment the draft did not list, do this
   after `submit` and before you tell the user. Write a proposals file
   with one item for it outside the repository, such as in a folder
   from `mktemp -d`, as
   [references/proposals-file.md](references/proposals-file.md) says,
   and save it with `add`, using the `review` the JSON shows as NAME.
   Give the item the next unused `id`, one above the highest `id` in
   the review's `round-N.json` files in
   `~/.local/state/pde/comment-review/NAME/` (read them only). To send
   a listed comment again, keep its `id` and `name`, with its current
   text in the file as `old`.

## When the tool is missing

If `pde-comment-review` is not on `PATH`, tell the user that the
comment review tool is not installed, and show the comments to the
user in the pull request or the chat instead.
