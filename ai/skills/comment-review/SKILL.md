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
writes the answers into the code and approves the comments, so the
comment checker stops later agents from changing them.

The tool supports Go comments today. Until it supports a language,
show that language's comments to the user in the pull request or
the chat instead.

## Steps

1. Leave the comments in the code as they are now; your wording goes
   in a proposals file. Write it outside the repository, such as in
   a folder from `mktemp -d`.
   [references/proposals-file.md](references/proposals-file.md) gives
   the format. In each file you list, put in one item for every
   comment that sits directly on a declaration, such as the package
   clause, a function, method, type, field, const or var, unless the
   user has approved it as it is now. The write approves those
   comments, so it refuses any the user has not seen. If you are
   unsure whether a comment is approved, list it as `keep`. Comments
   inside function bodies, or with a blank line after them, are
   never approved, so listing them is optional.
   - `old` is the comment as it is in the file now.
   - Propose a change as `reword`, with the new lines in `new`, or
     as `cut`.
   - A comment with no change is `keep`.
   - If a comment you must list has the same wording as another
     comment in its file, `add` refuses it. Before `add`, change an
     unapproved copy's wording in the code so they differ, or leave
     the whole file out of the review and show its comments in the
     pull request or the chat. Do the same for a file where a doc
     comment is followed by a Go directive, such as
     `//go:noinline`: the tool can never approve that comment.
2. Optional: before `add`, have a fresh reviewer, such as a new
   agent session given only the proposals file and the code, shorten
   or cut proposals the way the user's earlier notes asked.
3. Save it as the next round of a review. Pick a short name, such
   as the branch or package; it may use only `a-z`, `0-9`, `.`, `_`
   and `-`:

   ```bash
   pde-comment-review add shop /tmp/tmp.X1/round-1.json
   ```

   It prints `saved round 1 of review shop: REPO · BRANCH`. When it
   refuses the file, it names each problem and saves nothing; the
   reference lists them. The user's screen reads only reviews that
   `add` saved.
4. Tell the user the review is ready: "Review shop is ready: press
   Ctrl+b e in Herdr, or run `pde-comment-review open shop`." Do
   not run `open` yourself: it takes over the terminal it runs in.
   Do not edit the listed files before the last answer: the write
   finds each comment by its `old` text, and stops if one changed.
5. When the user says there are notes, read them:

   ```bash
   pde-comment-review notes shop
   ```

   It prints JSON with each note and the item it is about. Write a
   new proposals file with only those items, keeping their `id`,
   and save it with `add` under the same name. Only those items
   need a new answer; every other answer stays.
6. Never commit `.code-comments.json`, the tool's file at the repo root.

## When the tool is missing

If `pde-comment-review` is not on `PATH`, tell the user that the
comment review tool is not installed, and show the comments to the
user in the pull request or the chat instead.
