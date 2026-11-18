# What hub brief prints

Read this when you run `hub brief` to bring the user what waits on
them.

Its first line is `board active A of 8, review R of 6; K decisions`.
When cards wait on others, a `waiting-on-others W, oldest D days`
line follows. Then it prints one line per pull request approval
that waits on the user, read fresh from the request files, with its
reference, time before it expires, repository, number and action,
such as `approval A0016cd 1h12m left personal-dev-env #515 merge`.
`hub requests` lists the same reference. Then come
`reviews.md` and every entry, and last the "Waiting on others (no
action needed)" list, with each card's days in the stage, title and
reason. That list is news for the user, such as for a standup, never
a question, so do not repeat it in other messages unless the user
asks.

It exits 3, naming the entry, when one lacks its question, two
choices, `Recommended:` or `If no reply:`. Fix the entry and run it
again.

Show `reviews.md` as printed, and each entry in the short form of
`status-updates`: its title's words, question, then key; `(rec)` on
the `Recommended:` choice; its reason and other lines after the reply
line.
