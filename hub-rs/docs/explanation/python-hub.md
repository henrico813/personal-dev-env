# What changed from the Python hub

`hub` was first written in Python. The Rust `hub` replaced it, and
the Python hub was deleted. This page lists what works differently,
for people and skills that used the Python one.

## Versions and exit codes

- `hub --version` prints the version of the newest `hub-v` release
  tag, such as `hub 0.8.0`. The Python hub printed `0.1.0`.
- These commands exit 3, not 2, when they stop, so a script can tell
  a stop from a mistyped command: `go` and `gather` when no hub runs,
  `read` with no unread report, `handoff`, `brief` for an incomplete
  decision entry, and `auto check`, `auto approve` and `auto now`.
- `hub --machine LABEL alive` exits 2 with
  `alive works on this machine only`, the same words as `go`.

## The run folder

- `status`, `next`, `send`, `at`, `read`, `report`, `inbox`,
  `note`, `brief`, `handoff`, `adopt`, `clean` and `park` take the
  run folder from `--run DIR`, else from `HUB_RUN`, else from the one
  `~/.local/state/herdr-hub.*` folder whose `hub next` runs now. With
  no `hub next` running, they use the folder whose `events.jsonl`
  changed last and name it on stderr. When only auto runs have
  `hub next` running, they use the newest folder without
  `approved.md` the same way. With no folder found, two of the user's
  own hubs running, or a folder that does not exist, they exit 2.
  `board` needs no run folder.
- Every `auto` step takes the run folder from `--run DIR`, else from
  `HUB_RUN`.
- `read`, `handoff` and `adopt` add a line to `events.jsonl` in the
  run folder. `next` keeps its memory there, not in `next-seen.json`.
- `read --note` and `note` add a `## YYYY-MM-DD` heading before the
  first line of each day.

## Reports and notes

- `report NAME [FILE]` and `note TEXT` are new. `report` writes
  `report-NAME.md` whole, from FILE or standard input.
- `read --note TEXT --card [KEY]` also adds the note to the card's
  Log, with the key in the session's name or the KEY given. If a step
  after the archive fails, such as `card note`, the error starts with
  `archived report-NAME.HHMM.read, but`. The exit code is that failed
  step's own code.
- `brief`'s first line is
  `board active 5 of 8, review 19 of 6; 4 decisions`, or
  `board: REASON` when `card` fails. When nothing else waits, the
  board line is followed by `Nothing waits on the user.`, the same
  line the Python hub printed.
- `auto check` prints `hub: ` before its first problem only, one
  problem per line.

## Messages

- `send` sends at once to an OpenCode or Claude Code session that is
  working, idle or done, when the user is not in its pane. Another
  kind, such as Codex, still gets it only when idle or done.
- `send` holds a second message behind the first instead of exiting
  2, and `next` sends held messages oldest first. They wait in
  `sends/` in the run folder, not in `send-NAME.*` files.
- `send --file PATH` reads the message from a file. A message to a
  session that handed off goes to the newest live session on its
  card.
- "failed" is now "not sent". `send` exits 1 with
  `not sent NAME: REASON` when Herdr turned the message down or the
  session is gone, and with `unsure NAME: REASON; see sends/FILE`
  when it may have arrived.
- `inbox NAME` is new. It prints the messages held for NAME and
  counts each as delivered, so `next` never sends them.
- `at HH:MM` is new. It stores a message that `next` sends at that
  time today, to the `--to` sessions or, with `--all`, to every live
  worker.
- `send` to a parked session opens a new tab and resumes its
  OpenCode session there. It holds the message until the session
  starts.

## hub next

- `next` prints each report once per version. Its report, blocked,
  not-sent, unsure and timeout lines end with the hub's context, such
  as `(hub 131k)`. Once the hub reaches 150k, it prints
  `hand off now: hub at <tokens>` once instead. A newer `next` makes
  an older one print `replaced by a newer hub next` and exit.
  `--json` prints the same facts as JSON.
- `next` prints `old send file: PATH (hub 131k)` once for each old
  `send-NAME.*` file in the run folder, which it never sends, after
  every other line but `no hub handoff`.
- `next` runs a minute pass at most once a minute, and only after a
  session's state changed or a report was read. It reads
  `card board`. Each time active or review goes over its limit, it
  wakes the hub once with
  `board over its limit: active 10 of 8, review 18 of 6 (hub 131k)`.
  A board already over a limit at the run's first pass prints nothing.
- The minute pass never closes or parks a session. It lists each
  session that becomes `to-close` or `to-park` once, in one line per
  state and why, such as
  `to-close demo-kb-001-impl: DEMO-KB-001 accepted; run hub clean`.
  For each worker past 150k that is not `to-close` or `to-park`, it
  holds one handoff ask. It keeps its time and the finished sessions
  in `.minute-pass.json`.

## Closing and parking

- `clean` takes a run folder and runs the tidy pass. It closes the
  tab of each `to-close` session and parks each `to-park` session.
  Each change prints one line, such as
  `closed demo-kb-001-impl: DEMO-KB-001 accepted`.
- `clean` deletes the parked record of a session that is live again,
  and prints `forgot NAME`. Then it prints one `kept` line per kept
  session, and `nothing to close` when nothing changed. `--dry-run`
  prints `would close`, `would park` and `would forget` lines and
  changes nothing.
- A plain shell closes only with its finished agent's tab. A tab
  with only a shell stays open. The Python `clean` closed that tab;
  this one does not. While `card` or a request file cannot be read,
  `clean` closes nothing and prints `kept every session: REASON`.
  Closes and parks also go to `notes.md` and `events.jsonl`.
- `park NAME` is new. It parks one session whose state is `to-park`:
  it saves a record in `~/.local/state/pde/hub/parked/`, adds the
  session to its card, and closes its tab. It exits 3 with
  `NAME stays: REASON` otherwise.

## Hubs and handoffs

- `handoff` exits 3, not 2, when it stops, and starts the next hub's
  tab with `HUB_RUN` set. When the prompt to the next hub stalls, it
  does not press Enter to send pasted text, as the Python hub did.
- Session reviews are off by default. `next --start-reviews` starts
  the session review of an old hub that handed off, once that hub's
  tab has closed, in a new tab on the `rules.md` model at high
  effort. It also starts reviews still due from earlier handoffs in
  the run. It prints `review started NAME (hub 131k)`, or
  `review not started for OLD: REASON; start it from FILE with
  herdr-handoff (hub 131k)` and tries no more. Plain `next` starts no
  review.
- For an older hub of the run that no handoff names, `next` prints
  `no hub handoff for OLD -> hub adopt OLD (hub 131k)` once, and
  `status` lists the same words as a problem.
- `adopt OLD` is new. It makes the hub take over OLD, an older hub
  left open without `handoff`: it writes OLD's review handoff and
  records a handoff, so `clean` closes OLD once it is idle and
  `next --start-reviews` then starts its review.
- `launch NAME HANDOFF --model MODEL` is new. It starts a worker
  from its handoff file in the tab or workspace the sidebar rules
  pick, moves a ready card to active first, and records the handoff
  on the card and in `notes.md`.

## The sidebar and the screen

- `desk` and `board` are new. [hub commands](../reference/commands.md)
  describes them.
- `gather` also cuts Herdr's agent list down to the hubs and the
  sessions that wait on the user. The Python `gather` only moved
  hubs.
- `hub flag` on a working session marks it on idle and done, so
  "needs you" shows once it stops. The Python hub marked the working
  state, which showed "needs you" while the session still worked.

## hub status

- `status` shows each session's lifecycle state in its STATE column,
  not Herdr's status: `working`, `hub` (waits on the hub), `you`
  (waits on the user), `kept`, `to-close`, `to-park` or `parked`. The
  WHY column says why, such as `unread report`, `1 held` or the kept
  reason. It replaces the CARD, REPORT, SEND and LABEL columns.
- Above the table it prints the hub's own line, the board line, one
  count per state and the unread reports. Below it, one `no session:`
  line names active and review cards that no session works on. This
  line replaces the second table. `--json` prints the same facts as
  JSON.
- `status` looks for unsaved work only in linked worktrees. An agent
  that works in the main checkout gets no file check there, so its
  uncommitted files in the main checkout do not keep it.
