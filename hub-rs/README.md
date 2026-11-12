# hub in Rust

The `hub` command, rewritten in Rust. A hub is the Herdr session that
other sessions report to, and it runs `hub` to watch, message and
tidy them. The Python hub in `../hub/` stays the one agents and skills
use until this program replaces it.

pde-installer builds this folder on the Development tier and installs
it as `~/.local/bin/hub-next`, so you can try it beside the Python
`hub`. So far it has `status`, `next`, `send`, `read`, `report`,
`inbox`, `note`, `brief`, `handoff`, `clean`, `park`, `auto`, `flag`,
`unflag`, `alive`, `go`, `gather`, `desk` and `board`.

```bash
make build   # target/release/hub
make check   # format, lint and test, as CI does
make test
```

Exit codes: 0 done, 1 an external system such as Herdr failed, 2 wrong
usage, 3 not done, such as when no hub is running. Each failure prints
its reason. When the reader of the output stops early, as
`hub-next go | head` does, the hub stops printing and keeps its exit
code.

## Differences from hub

- `hub-next --version` prints `hub 0.2.0`, since the binary already
  calls itself `hub`. The Python hub prints `hub 0.1.0`.
- `hub-next go` and `hub-next gather` exit 3, not 2, when no hub is
  running, so a script can tell this from a mistyped command.
- `hub-next flag` on a working session marks it on idle and done, so
  "needs you" shows once it stops. The Python hub marked the working
  state, which showed "needs you" while the session still worked.
- `status`, `next`, `send`, `read`, `report`, `inbox`, `note`,
  `brief`, `handoff`, `clean` and `park` take the run folder from
  `--run DIR`, else from `HUB_RUN`. With neither, or with a folder
  that does not exist, they exit 2.
- `read` exits 3, not 2, when the session has no unread report.
  `read --note TEXT --card [KEY]` also adds the note to the card's Log,
  with the key in the session's name or the KEY given. If a step
  after the archive fails, such as `card note`, the error starts with
  `archived report-NAME.HHMM.read, but`. The exit code is that failed
  step's own code.
- `read --note` and `note` add a `## YYYY-MM-DD` heading before the
  first line of each day.
- `report NAME [FILE]` and `note TEXT` are new. `report` writes
  `report-NAME.md` whole, from FILE or standard input.
- `read` and `handoff` add a line to `events.jsonl` in the run folder.
- `handoff` exits 3, not 2, when it stops, and starts the next hub's
  tab with `HUB_RUN` set. When the prompt to the next hub stalls, it
  does not press Enter to send pasted text, as the Python hub did.
- `brief` exits 3, not 2, for an incomplete decision entry. Its first
  line is `board 24/8, review 19; 4 decisions`, or `board: REASON`
  when `card` fails. When nothing else waits, the board line is
  followed by `Nothing waits on the user.`, the same line the Python
  hub printed.
- `auto check`, `auto approve` and `auto now` exit 3, not 2, when
  they stop, such as for a reply that approves nothing. Every `auto`
  step takes the run folder from `--run DIR`, else from `HUB_RUN`.
  `auto check` prints `hub: ` before its first problem only, one
  problem per line.
- `hub-next --machine LABEL alive` exits 2 with
  `alive works on this machine only`, the same words as `go`.
- `send` sends at once to an OpenCode or Claude Code session that is
  working, idle or done, when the user is not in its pane. Another
  kind, such as Codex, still gets it only when idle or done.
- `send` holds a second message behind the first instead of exiting
  2, and `next` sends held messages oldest first. They wait in
  `sends/` in the run folder, not in `send-NAME.*` files.
- `send --file PATH` reads the message from a file. A message to a
  session that handed off goes to the newest live session on its card.
- "failed" is now "not sent". `send` exits 1 with
  `not sent NAME: REASON` when Herdr turned the message down or the
  session is gone, and with `unsure NAME: REASON; see sends/FILE` when
  it may have arrived.
- `inbox NAME` is new. It prints the messages held for NAME and counts
  each as delivered, so `next` never sends them.
- `next` prints each report once per version. Its report, blocked,
  not-sent, unsure and timeout lines end with the hub's context, such
  as `(hub 131k)`. Once the hub reaches 150k, it prints
  `hand off now: hub at <tokens>` once instead. A newer `next` makes
  an older one print `replaced by a newer hub next` and exit. `--json`
  prints the same facts as JSON. It keeps its memory in
  `events.jsonl`, not in `next-seen.json`.
- `next` prints `old send file: PATH (hub 131k)` once for each old
  `send-NAME.*` file in the run folder, which it never sends, after
  every other line.
- `next` runs a minute pass at most once a minute, and only after a
  session's state changed or a report was read. It reads `card board`.
  Each time the board crosses into its limit, it wakes the hub once
  with `board over its limit: 25/8 (hub 131k)`. A board already over
  its limit at the run's first pass prints nothing. It never closes or
  parks a session. It lists each session that becomes `to-close` or
  `to-park` once, in one line per state and why, such as
  `to-close demo-kb-001-impl: DEMO-KB-001 accepted; run hub clean`.
  For each worker past 150k that is not `to-close` or `to-park`, it
  holds one handoff ask. It keeps its time and the finished sessions
  in `.minute-pass.json`.
- `clean` takes a run folder and runs the tidy pass. It closes
  the tab of each `to-close` session and parks each `to-park` session.
  Each change prints one line, such as
  `closed demo-kb-001-impl: DEMO-KB-001 accepted`. It deletes the
  parked record of a session that is live again, and prints
  `forgot NAME`. Then it prints one `kept` line per kept session, or
  `nothing to close` when nothing changed. `--dry-run` prints
  `would close`, `would park` and `would forget` lines and changes
  nothing. A plain shell closes only with its finished agent's tab. A
  tab with only a shell stays open. The Python `clean` closes that
  tab; this one does not. While `card` or a request
  file cannot be read, it closes nothing and prints
  `kept every session: REASON`. Closes and parks also go to
  `notes.md` and `events.jsonl`.
- `park NAME` is new. It parks one session whose state is `to-park`: it
  saves a record in `~/.local/state/pde/hub/parked/`, adds the
  session to its card, and closes its tab. It exits 3 with
  `NAME stays: REASON` otherwise.
- `board` is new. It shows what waits on the user, which cards are in
  review and which cards are worked on now, sized for a 40x16 phone
  screen. It reads again every 10 seconds or when r is pressed. The
  top line counts what waits, such as
  `YOU 3+2  PR 1  HUB 2  busy 9/8!  10:42`. Sections follow in this
  order: problems, needs you, in review, working on now, pull
  requests, later, optional, PR notes, waiting on the hub, working
  sessions, parked, safe to close and kept. Needs you, in review,
  working on now and pull requests start open. The session sections
  and the other side sections start folded.
  In review (mark `V`) shows each card's project and title, then its
  key and the one step the user takes next. That step is the card's
  newest Log line that starts `next: `, such as
  `next: review PR #498`, else `review PR #N` from its newest PR
  link, else `read plan KEY` from its first plan, else
  `open the card`. Working on now lists first, marked `!`, a draft or
  ready card that a session works on, since its stage is out of date.
  Active cards follow, `*` with the sessions that work on them and
  `-` with no worker. Approval waits come
  first under needs you. A `decisions.md` entry with a
  `Not before: 2026-11-11 20:00` line waits under later until that
  local time, and one with `Priority: optional` waits under optional.
  Only a `reviews.md` table row whose first cell is a PR number, such
  as `| #510 | install on WSL | worker |`, counts as a pull request,
  in file order. Other rows and paragraphs go to PR notes.
  Rows wider than the screen wrap onto more lines. One tap on a
  heading folds or opens it, and an opened heading moves to the top.
  One tap chooses an item and a second tap opens its full text. Any
  tap leaves the details or the help. The wheel or a swipe scrolls
  the list. The bottom
  line's thirds page up, open the menu of sections and page down. A
  tap on a section in the menu opens it at the top of the list, and
  the menu's last row opens a page of keys and taps. Keys are j, k,
  the arrows, the page keys, g and G, n and N for sections, Enter or
  i to open, s for the menu, ? for the help, Esc to go back and q to
  quit. `NO_COLOR` turns colors off.
  `board --print` prints one screen as text, 40x16 unless `--size`
  says otherwise.
- `send` to a parked session opens a new tab and resumes its OpenCode
  session there. It holds the message until the session starts.
- `status` shows each session's lifecycle state in its STATE column,
  not Herdr's status: `working`, `hub` (waits on the hub), `you`
  (waits on the user), `kept`, `to-close`, `to-park` or `parked`. The
  WHY column says why, such as `unread report`, `1 held` or the kept
  reason. It replaces the CARD, REPORT, SEND and LABEL columns. Above
  the table it prints the hub's own line, the board line, one count
  per state and the unread reports. Below it, one `no session:` line
  names active and review cards that no session works on. This line
  replaces the second table. `--json` prints the same facts as JSON.
- `status` looks for unsaved work only in linked worktrees. An agent
  that works in the main checkout gets no file check there, so its
  uncommitted files in the main checkout do not keep it.
