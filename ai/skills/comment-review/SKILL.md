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

The tool supports Go comments today. Until it supports a language,
show that language's comments to the user in the pull request or
the chat instead.

## Steps

1. Never edit a comment the user approved, even when a plan says
   to: propose your change in step 3. Optional: before `draft`, have
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
   user can only drop. It saves them as round 1 of a new review
   named after the branch, prints `saved round 1 of review NAME: REPO
   · BRANCH`, then prints the round as JSON. If it says no comment
   needs the user's answer, tell the user there is nothing to review.
   When it refuses, it names each problem and saves nothing; fix them
   and run it again. If a comment has the same wording as another
   comment in its file, or as the approved text of one you edited,
   change an unapproved copy's wording in the code. If a round waits
   for the user, use step 3 or step 5 instead, taking the ids from
   the latest `round-N.json` in
   `~/.local/state/pde/comment-review/NAME/` (read it only), or wait
   for the answers and run `draft` again. Tell the user when the
   waiting review is for another checkout.
3. Optional: to propose other wording, write a proposals file with
   only the changed items, outside the repository, such as in a
   folder from `mktemp -d`.
   [references/proposals-file.md](references/proposals-file.md) gives
   the format. Copy a drafted item from the printed JSON, keep its
   `id` and `name`, and change `proposal` and `new`. To change an
   approved comment the draft did not list, add an item with the
   next unused `id` and the approved text as `old`. If `add` says an
   approved comment is gone because you deleted its code, add an item
   for it with its `name`, `cut`, `code_removed`, and as `old` the
   approved text the error prints, also in `.code-comments.json`
   (read it only). Save it under the review's name, such as shop:

   ```bash
   pde-comment-review add shop /tmp/tmp.X1/round-1.json
   ```

4. Tell the user the review is ready: "Review shop is ready: press
   Ctrl+b e in Herdr, or run `pde-comment-review open shop`." Do
   not run `open` yourself: it takes over the terminal it runs in.
   Do not edit the listed comments before the last answer: the write
   refuses an edited one or puts the user's answer over your edit.
5. When the user says there are notes, read them:

   ```bash
   pde-comment-review notes shop
   ```

   It prints JSON with each note and the item it is about. Write a
   new proposals file with only those items, keeping their `id` and
   `name`, and save it with `add` under the same name. Only those
   items need a new answer; every other answer stays. If the user
   asks for removed code back, put it back with its approved comment,
   and send the item without `code_removed`, proposing `keep`.
6. Never commit `.code-comments.json`, the tool's file at the repo root.
7. Never edit a review's saved files in
   `~/.local/state/pde/comment-review/`: change a review only with
   `draft` and `add`, and leave the answers and notes to the user.

## When the tool is missing

If `pde-comment-review` is not on `PATH`, tell the user that the
comment review tool is not installed, and show the comments to the
user in the pull request or the chat instead.
