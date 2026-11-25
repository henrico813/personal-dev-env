# What hub brief and hub waiting print, and hub ask takes

Read this when you run `hub brief` to bring the user what waits on
them, when you end a message with `hub waiting`, or when you write a
question for `hub ask`.

Its first line is `board active A of 8, in-review R of 6; K questions`.
When cards are blocked, a `blocked W, oldest D days`
line follows. Then it prints one line per pull request approval
that waits on the user, read fresh from the request files, with its
reference, time before it expires, repository, number and action,
such as `approval A0016cd 1h12m left personal-dev-env #515 merge`.
`hub requests` lists the same reference. Then come
`reviews.md` and every card's open question, each starting
`Question KEY: TITLE`, and last the "Blocked (no action needed)"
list, with each card's days in the stage, title and reason. That
list is news for the user, such as for a standup, never a
question, so do not repeat it in other messages unless the user
asks.

When `card` cannot list the questions, the first line ends
`questions unread` and a `questions: REASON` section says why. Tell
the user that questions may wait, and fix the reason or run it
again.

A `Check first: the user wrote in NAME TIME` line under a question's
title names a session on its card where the user typed on or after
the day the question was asked. Read that session's pane before you
run `hub waiting`. When the user answered there, run `hub answer`
with that answer, so `hub waiting` leaves the question out. Never
show the line to the user.

Show `reviews.md` as printed. Leave the questions out of your own
text, since the `hub waiting` output that ends each message shows
them.

When you release a draft, as SKILL.md's Cards section allows, record it
with a `hub note` line that starts `released KEY`. `hub brief` does not
list them. After `hub brief`'s own parts, add a "Released by the hub"
list. It covers the `released` lines in `notes.md` after the last
`brief shown` line, or all of them when there is no `brief shown` line
yet. Each list line holds the card's key and its current title and
stage. Take the title and stage from `card show KEY`. Leave the list out
when it would be empty. It is news for the user, not a question. Then
add a `hub note` line `brief shown`.

## End each message with hub waiting

SKILL.md's "Questions for the user" says when to run
`hub waiting --run "<run folder>"` and that its output ends every
message, word for word.

- It prints each question whole, oldest first, with the number its
  reply key uses, such as `Q2A`. A number stays until no numbered
  question waits, so match a reply to the number the output gave.
  Numbers may pass `Q9`, since every open question shows.
- It also shows the count on your sidebar row, such as
  `hub free, 2 for you`.
- Check your own text with the `status-updates` checker before you
  add the output, since its form is not the checker's.
- Ask about a reply, such as which question it answers, in one plain
  sentence with no number, so its number never clashes with one that
  `hub waiting` gives.

## A question file

A `hub ask <key> <question file>` file looks like this:

    ## New model overnight

    Deploy the new model tonight so you can test it in the morning?

    - A. Deploy from the branch, with the undo path ready.
    - B. Wait until you return.

    Recommended: A, since you want to test it in the morning.
    If no reply: nothing deploys; the worker goes on with its tests.
    Depends on: your rule of 2026-11-09 to test it in the morning.

The `If no reply:` line says what stays blocked and what the worker
does meanwhile. Add `Depends on:` when the question rests on an
earlier decision, and name who made it.

## What hub ask refuses and marks

`hub ask` refuses a question with a part missing. It also refuses a
card that already waits on the user, since a card shows one open
question. Keep the next question for that card in `notes.md`, and ask
it with `hub ask` right after `hub answer`. `hub ask` marks the card
needs-you, so `hub status` shows the card's sessions as `you`.
