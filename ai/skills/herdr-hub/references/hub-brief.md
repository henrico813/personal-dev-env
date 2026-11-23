# What hub brief prints and hub ask takes

Read this when you run `hub brief` to bring the user what waits on
them, or when you write a question for `hub ask`.

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
show the question. When the user answered there, run `hub answer`
with that answer and leave the question out. Never show the line to
the user.

Show `reviews.md` as printed, and each question in the short form of
`status-updates`: its title's words, question, then key; `(rec)` on
the `Recommended:` choice; its reason and other lines after the reply
line. Number them, and hold any past `Q9`, as "Question numbers" in
`status-updates` says.

## A question file

A `hub ask <key> <question file>` file looks like this:

    ## New model overnight

    Deploy the new model tonight so you can test it in the morning?

    - A. Deploy from the branch, with the undo path ready.
    - B. Wait until you return.

    Recommended: A, since you want to test it in the morning.
    If no reply: nothing deploys; the worker goes on with its tests.
    Depends on: your rule of 2026-11-09 to test it in the morning.

## What hub ask refuses and marks

`hub ask` refuses a question with a part missing. It also refuses a
card that already waits on the user, since a card shows one open
question. Keep the next question for that card in `notes.md`, and ask
it with `hub ask` right after `hub answer`. `hub ask` marks the card
needs-you, so `hub status` shows the card's sessions as `you`.
