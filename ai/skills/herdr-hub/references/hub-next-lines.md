# Rare hub next lines

Read this when `hub next` prints a line that SKILL.md's Wake on news
does not list, or when a session runs on another machine.

- `not sent <name>: <reason>`: Herdr turned a held message down, or
  the session is gone. The message stays in a `.notsent` file in
  `<run folder>/sends/`. Check the session, and send it again. Once
  you have sent it again, or decided not to, delete that `.notsent`
  file, since the session waits on you while it stays.
- `unsure <name>: <reason>`: a held message may have arrived. Handle
  it as an `unsure` send, as SKILL.md's Send to a session says.
- `timed message to <N> sessions: <path>`: a message stored with
  `hub at` has reached its time and is held for N sessions. `<path>` is
  the `--file` given to `hub at`. A session that is gone or turns it
  down gets its own `not sent` or `unsure` line.
- `board over its limit: active N of 8, review N of 6`: the cards
  now in active or in review passed that stage's limit. Tell the
  user at once, with the board line, and propose cards to pause as
  SKILL.md's Cards says. Start no ready card until both counts are
  back within their limits or the user says to go on.
- `old send file: <path>`: handle it as the `hub status` line of the
  same name in SKILL.md's The sessions.
- `no hub handoff for <old> -> hub adopt <old>`: an older hub of this
  run is still open, and no `hub handoff` started you from it. Run
  `hub adopt --run "<run folder>" <old>`. It writes the old hub's
  review handoff. Once the old hub is idle, `hub next` lists it as
  `to-close`, and `hub clean` closes its tab.
- `review started <name>`: with `--start-reviews`, the old hub's
  session review now runs. Bring its pick list to the user when it
  reports.
- `review not started for <old>: <reason>; start it from <file> with
  herdr-handoff`: `hub next` tries no more. Start that review with
  `herdr-handoff` from that file.

## Sessions on another machine

A session started with `herdr --machine <label>` runs on that
machine's Herdr server, and you cannot read its files, so `hub next`
does not see it. Write in its handoff that it ends each turn with its
report. Learn of the turn's end with one background command, and read
the report with the second:

    herdr --machine <label> agent wait <name> --until idle \
      --until done --until blocked
    herdr --machine <label> agent read <name> \
      --source recent-unwrapped --lines 80

`hub --machine <label> status --run "<run folder>"` lists that
machine's sessions.
