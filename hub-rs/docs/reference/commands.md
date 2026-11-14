# hub commands

`hub --help` lists the commands, and `hub COMMAND --help` gives each
one's flags and an example. This page adds what each one prints and
when it exits with a code other than 0.

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | Done. The output lines go to standard output. |
| 1 | A tool such as Herdr, `card`, git or a file failed. |
| 2 | Wrong usage, such as a bad name or no run folder. |
| 3 | Not done, for the reason given. |

Each failure prints its reason to standard error, starting with
`hub: `. Usage mistakes caught before the command runs start with
`error:` instead. When a later step fails after an earlier one
finished, the reason reads `hub: DONE, but REASON`, such as
`hub: archived report-docs-fix.2214.read, but ...`, and the exit code
is the failed step's code.

## The run folder

The commands marked `*` in `hub --help` read the run folder from
`--run DIR`, else from `HUB_RUN`, else from
`~/.local/state/herdr-hub.*`. `--run` goes after the command name,
such as `hub note --run /tmp/run "merged #9"`.

Without `--run` or `HUB_RUN`, they take the one folder there whose
`hub next` runs now. When two or more run, they skip auto runs, which
have an `approved.md`. When none runs, they take the folder whose
`events.jsonl` changed last, and print
`hub: no hub next is running, so using DIR` to standard error.

They exit 2 with:

- `no run folder at DIR` for a folder that does not exist.
- `no run folder; no hub here wrote events yet, so pass --run DIR`
  when no folder is found.
- `no run folder; N hubs run here, so pass --run with one of DIRS`
  when more than one hub runs.

The `auto` steps never search. They take `--run DIR`, else
`HUB_RUN`, and exit 2 with
`no run folder; pass --run DIR or set HUB_RUN` without either.

The folder holds these files, among others:

- `report-NAME.md`: a worker's unread report.
- `report-NAME.HHMM.read`: a report the hub read.
- `notes.md`: the hub's log, one `- HHMM text` line per entry under a
  `## YYYY-MM-DD` heading for each day.
- `events.jsonl`: what `hub` has done, one JSON object per line.
- `sends/`: messages that wait or may not have arrived.
- `timed/`: messages that `hub at` stores until their time.
- `waits/`: one file per worker that waits on a long job.

## --machine

`hub --machine LABEL COMMAND` sends Herdr calls to the SSH machine
saved in Herdr as LABEL. Only `status`, `send`, `flag` and `unflag`
take it. The others exit 2 with `COMMAND works on this machine only`.

## Environment

- `HUB_RUN`: the run folder, when `--run` is not given.
- `HUB_POLL_SECONDS`: how often `next`, `watch` and `handoff` check,
  default 5. A value that is not a number exits 2.
- `HERDR_SOCKET_PATH`: Herdr's socket, else
  `~/.config/herdr/herdr.sock`.

## Commands for workers

### report

`hub report [--run DIR] NAME [FILE]` writes `report-NAME.md` whole,
from FILE, or from standard input when FILE is `-` or left out.

- Prints `report NAME written, N lines`.
- Exits 3 with `empty report; nothing written` for empty text.
- Exits 2 with `NAME is not an agent name` for a bad name.
- Adds `hub watch restarted` when the run has a hub and no watch
  runs, or `hub watch did not restart: REASON`.

### inbox

`hub inbox [--run DIR] NAME` prints the messages held for NAME and
counts each as delivered, so `next` never sends them.

- Prints `no held messages for NAME`, or `N held messages for NAME`
  followed by a `--- HH:MM` line and the text of each message.

### wait

`hub wait [--run DIR] NAME (--pid PID | --file PATH) --deadline
MINUTES` saves a wait in `waits/NAME.json` and returns at once. The
watch prods NAME once the process ends, the file changes or the
deadline passes. A new wait replaces the old one.

- Prints `hub watch prods NAME once pid PID ends or at HH:MM; end
  your turn now`, or the same with `PATH changes`.
- Exits 2 with an `error:` line unless exactly one of `--pid` and
  `--file` is given.
- Adds the same watch line as `report`.

The prod starts with `hub watch: `, such as
`hub watch: your wait ended, since pid 4242 ended. Check the job and
go on.`

## Commands for the hub

### status

`hub status [--run DIR] [--json]` prints the hub's line, the board,
the count per state, the unread reports and one row per session:

```text
hub kanban-hub-18 131k, watcher running
board 24/8, review 19
open 6: 1 working, 1 hub, 1 you,
  1 kept, 1 to-close, 1 to-park; 1 parked
unread 2: demo-kb-007-impl docs-fix

NAME                  STATE     CTX   WHY
docs-fix              working   152k  1 held, hand off
demo-kb-007-impl      hub       98k   unread report
demo-kb-009-auto      you       12k   blocked
demo-kb-012-auto      kept      77k   unpushed in /home/u/wt/demo-kb-012
review-kanban-hub-16  to-close  60k   report read
demo-kb-090-auto      to-park   104k  -
demo-kb-081-auto      parked    -     since 11-10 22:41

no session: DEMO-KB-004 active
```

- The hub's line adds `: hand off now` after its context at 150k or
  more.
- The board line is `board: REASON` when `card` fails.
- STATE is one of `working`, `hub` (waits on the hub), `you` (waits
  on the user), `kept` (finished, but WHY says what keeps it open),
  `to-close`, `to-park` and `parked` (no tab, can reopen).
- `no session:` names active and review cards that no session works
  on.
- `--json` prints the same facts as one JSON object.

### next

`hub next [--run DIR] [--timeout SECONDS] [--json]
[--start-reviews]` waits until something needs the hub, prints one
line and exits 0. While it waits, it sends held messages. At most
once a minute it reads the board, finds finished sessions and asks
each card's worker at 150k or more to hand off. It never closes or
parks a session.

- `report NAME (hub 131k)`: NAME wrote a report.
- `missed report NAME (hub 131k)`: the report waited 10 minutes with
  no `next` line.
- `blocked NAME (hub 131k)`: NAME waits for the user.
- `not sent NAME: REASON (hub 131k)`: a held message did not arrive.
- `unsure NAME: REASON (hub 131k)`: a held message may have arrived.
- `timed message to N sessions: PATH (hub 131k)`: a message stored
  with `at` reached its time and is held for each session. PATH is
  the `--file` given to `at`.
- `board over its limit: 25/8 (hub 131k)`: too many cards are active
  or in review.
- `hand off now: hub at 152k`: the hub should hand off now.
- `to-close NAMES: WHY; run hub clean`, or the same with `to-park`:
  sessions that finished, listed once.
- `no hub handoff for OLD -> hub adopt OLD (hub 131k)`: an older hub
  of the run is open, and no `handoff` started this hub from it.
- `old send file: PATH (hub 131k)`: a message file the Python hub
  left, which `next` never sends.
- `review started NAME (hub 131k)`, or
  `review not started for OLD: REASON; start it from FILE with
  herdr-handoff (hub 131k)`: only with `--start-reviews`, the session
  review of an old hub whose tab closed.
- `replaced by a newer hub next`: another `next` took over.
- `timeout (hub 131k)`: nothing came before `--timeout`.

Each report version, blocked state, timed message and board crossing
prints once.
`(hub -)` means the hub's context is unknown. `--json` prints one
object with the same facts:

```json
{"news":"report","name":"docs-fix","detail":null,"hub_context_tokens":131204}
```

### send

`hub send [--run DIR] NAME (MESSAGE | --file PATH)` sends now when
the session can take it, and otherwise holds the message for `next`.

- `sent NAME`: delivered.
- `held NAME: WHY`: held, where WHY is `it is blocked`,
  `the user is in its pane`, `1 older message waits`,
  `KIND takes messages when idle` or `its state is unknown`.
- `reopening NAME; held until it starts`: NAME was parked.
- `sent OTHER (NAME handed off to it)`: NAME handed off on its card.
- Exits 1 with `not sent NAME: REASON`, or with
  `unsure NAME: REASON; see sends/FILE` when it may have arrived.

### at

`hub at [--run DIR] HH:MM (--to NAME... | --all) --file PATH` stores
the message in `timed/` for HH:MM today, in this machine's local
time. At that time, `next` holds it for each session and sends it as
it sends other held messages. `--all` means every live worker found
at that time: a session on a card or that has reported, never a hub.
Deleting the stored file cancels the message.

- `at 20:00 to NAMES: FILE`, or `at 20:00 to every live worker: FILE`:
  stored, where FILE is the stored file.
- Exits 2 with `20:00 has passed today; give a later time` for a time
  not after now.

### read

`hub read [--run DIR] NAME [--note TEXT [--card [KEY]]]` prints
NAME's report, renames it to `report-NAME.HHMM.read`, and ends with
`unread: NAMES` or `unread: -`.

- `--note TEXT` adds a timed line to `notes.md`.
- `--card` also adds the note to the card's Log, with the key in
  NAME or the KEY given.
- Exits 3 with `no unread report from NAME`.
- Exits 2 with `NAME holds no card key; pass --card KEY`.

### note

`hub note [--run DIR] TEXT` adds `- HHMM TEXT` to `notes.md`, after a
date heading on the day's first line. Prints `noted HHMM`.

### brief

`hub brief [--run DIR]` prints what waits on the user. The first line
is `board 24/8, review 19; 4 decisions`, or `board: REASON`. Then come
`reviews.md` and one `Decision: TITLE` section per entry in
`decisions.md`, or `Nothing waits on the user.`

- Exits 3 with `decisions.md entry 'TITLE' lacks ...` for an
  incomplete entry.

### handoff

`hub handoff [--run DIR] NAME FILE` starts the next hub, named NAME,
in a new tab with `HUB_RUN` set, and gives it FILE to follow. Run it
in the hub's own pane. Prints:

```text
started NAME on MODEL; its first reply ran at EFFORT
review handoff PATH
```

- Exits 3 before it starts anything when `rules.md` has no
  `--model '...'` line, FILE does not exist, or it runs outside the
  hub's pane.
- Exits 3 with `started NAME, but REASON` when a step after the start
  failed. Do not run it again, since the next hub already runs.

### adopt

`hub adopt [--run DIR] OLD` takes over OLD, an older hub of the run
that no `handoff` named. It writes OLD's review handoff and records a
handoff, so `clean` closes OLD once it is idle. Run it in the hub's
own pane. Prints `adopted OLD; hub clean closes its tab once it is
idle, then hub next --start-reviews starts review-OLD`.

- Exits 3 with `OLD is not an older hub left without hub handoff`.

`handoff` and `adopt` also move the hubs to the top of the sidebar,
as `gather` does. When that fails, they add
`could not put the hubs on top: REASON; hub gather tries again`.

### launch

`hub launch [--run DIR] NAME HANDOFF --model MODEL [--workspace LABEL]`
starts a worker named NAME on MODEL in the current folder, prompts it
to read HANDOFF, and records the start. A card key at the start of
NAME, such as `demo-kb-007-impl`, makes it that card's worker. The
worker opens in a new tab of the workspace whose label starts with
the key, else in a new workspace such as `demo-kb-007 impl`. Work
with no card opens in the hub's workspace, so run it in the hub's
own pane. `--workspace LABEL` opens it in that workspace, and makes
the workspace if none has that label. A ready card moves to active
first, so `card` checks the work limit before anything starts. After
the start, the card gets a `Handoff PATH` note and the worker's
session, and `notes.md` gets a line. Prints:

```text
started NAME (PANE) on MODEL
new workspace LABEL
```

The second line comes only when it made a workspace.

- Exits 3 before it changes anything when HANDOFF does not exist, a
  live agent is named NAME, `card list` shows no card for the key, or
  work with no card runs outside the hub's pane.
- Exits 1 with `card`'s message, and changes nothing, when `card`
  refuses the move to active, such as past the work limit.
- A failure after the move begins `moved KEY to active, but REASON`.
  One after the start begins `started NAME` and lists each later step
  that finished, such as `started NAME, sent the handoff prompt,
  added the card note, but REASON`, so the hub does the rest by hand.
  Running it again after a start refuses, since NAME already runs.

### watch

`hub watch start [--run DIR]` names this hub as the run's hub and
starts the watch when none runs on this machine. Run it once in the
hub's own pane. Prints `hub watch started; it watches the workers of
NAME`, or `hub watch runs; it watches the workers of NAME`.

- Exits 3 with `hub watch reads only DIR/herdr-hub.* folders` for any
  other run folder, or `hub watch run did not start: REASON`.

The watch is one program per machine, outside every chat. Every 5
seconds it reads each `~/.local/state/herdr-hub.*` run that has a
hub. It prods a worker that sat idle 10 minutes owing a report, once
in that state, and a worker whose wait ended. It marks a blocked
worker "needs you". When a report waits 10 minutes with no `next`
line, the hub's next `next` prints `missed report NAME`. When the
run's hub is gone, it notifies the phone instead, for reports from
the last day. It never types into a hub's pane. Its lock and log are
in `~/.local/state/pde/hub/`. The `hub-watch` systemd user unit keeps
it running on full installs.

The watch also reads `~/.local/state/pde/update.json`, which
`pde-installer update` writes. For each new `failed` or `skipped`
result it sends the phone one notification, titled
`pde-installer update failed` or `pde-installer update skipped`, with
the reason. The same result again sends nothing, even after a restart,
since the watch keeps the last result in `hub/update-seen.json`.

### clean

`hub clean [--run DIR] [--dry-run]` closes the tab of each `to-close`
session and parks each `to-park` session. It looks at every session
in Herdr on this machine. Only `clean` and `park` close tabs.

- `closed NAME: WHY` and `parked NAME: KEY review`.
- `forgot NAME`: a parked session that is live again.
- `kept NAME: WHY` for each kept session, and for a finished one
  whose tab also holds other work, with `its tab holds other work`.
- `nothing to close` when nothing changed.
- `kept every session: REASON` when `card` or a request file cannot
  be read.
- `--dry-run` prints `would close`, `would park` and `would forget`
  and changes nothing.

### park

`hub park [--run DIR] NAME` parks one `to-park` session: it saves a
record, adds the session to its card and closes its tab. Prints
`parked NAME on KEY`.

- Exits 3 with `no session NAME`, or `NAME stays: REASON`.

### board

`hub board` shows four panels, sized for a 40x16 phone screen.
Panel 1, the queue, shows the cards by stage: In progress, In
review, Needs you, Ready and Draft. The cards come from
`card list`. Needs you holds the questions in each `decisions.md`
and the sessions that wait on the user, from every hub on this
machine that Herdr lists: each `~/.local/state/herdr-hub.*` folder
whose hub `hub watch start` named, auto runs included. Each
question's tag is its hub's name, cut to 6 letters, and its
details page names the hub in full. With no hub running, Needs you
shows `no hub running`. It ignores `HUB_RUN`, and the hidden
`--run DIR` shows one run only. A Could not read group shows on
top when a read fails. An open group shows four rows, then
`+ N more`; Ready and Draft start folded. Each row starts with a
mark for its group: `*` in progress, `@` in review, `?` needs you,
`o` ready, `.` draft and `!` could not read.

Panel 2, kanban, lists the cards by stage: REVIEW, ACTIVE, READY
and DRAFT. Panel 3, sessions, groups every session as `hub clean`
decides: NEEDS YOU, WORKING, KEPT, SAFE TO CLOSE and PARKED. Each
row shows the name, a context bar full at 150K tokens, the context
in thousands and why the session is in its group, such as
`unpushed review`. A why too long for the row goes on the line
below it. Panel 4, machine, shows the file-watch slots in
use (`WATCH`, with `high` from 90%) and free disk (`DISK`), then
the six newest reports. Tab or 1 to 4 picks a panel. Below 100
columns the board shows one panel, and its hint starts with the
panel number, such as `2/4`. From 100 columns it shows all four,
and `>` marks the panel keys act on.

- On the queue, a tap on a count jumps to its group, a tap on a
  heading folds or opens it, and a tap on a row opens its details.
  On a details page, any tap, Enter, space, q or Esc goes back.
- On the queue, j, k, the arrow and page keys or the wheel move the
  choice, and Enter or space acts on it. On kanban and machine they
  scroll. r reads again, and q quits from the list. It also reads
  again every 10 seconds. `NO_COLOR` turns colors off.
- On the sessions panel, a tap on a row only chooses it. The bar
  at the bottom then reads `-> jump to NAME`, and a tap on it
  shows that session in Herdr. Enter also jumps, but only after
  0.7 seconds with no input, so a stray Enter after a tap does not
  jump. A failed jump shows `jump failed: REASON` on the bottom
  line.
- c, from any panel, lists the SAFE TO CLOSE sessions, each marked
  `close` or `park`, and ends in `+ N more` when they do not fit.
  The list does not change while it is open. y, once the list has
  been open 1 second and no key came for 0.7 seconds, closes and
  parks the listed sessions as `hub clean` does and shows its
  output, with a `kept` line for any it now keeps. Any other key or
  a tap cancels. With nothing to close or no hub running, c says so
  and changes nothing.
- `--print` prints one screen as text, and `--dump` prints its
  elements as one JSON line. Both draw a new board from the run
  folder, at `--size`, default 40x16.
- `--listen` shows the live board and also takes steps from
  `board.sock` in the run folder. From another terminal,
  `hub board --send 'STEP'` does one step on it and prints the screen
  that follows as one JSON line. It exits 2 when the board cannot
  follow the step, and 3 when no board listens. The socket works on
  Linux and macOS, not Windows.
- A step is `tap ID`, `tap COLUMN ROW`, `press KEY`, `wheel up` or
  `wheel down`. KEY is `enter`, `esc`, `space`, `tab`, `up`,
  `down`, `pageup`, `pagedown`, `ctrl-c` or one character, such as
  `q`. The hidden `--control`, which tests use, takes the same
  steps but never jumps or cleans.

`hub board --help` describes the JSON fields. "Screens agents can see
and tap" in the [README](../../README.md) gives the rules every such
screen follows.

### auto

The steps of an auto run, in order. Each takes `--run DIR`.

- `hub auto ready` writes the ready cards to `ready.md` and prints
  them, or `No ready cards.`
- `hub auto check` checks `run.md` and prints
  `run N: K cards to run`. Exits 3 with one line per problem.
- `hub auto approve N REPLY` saves `approved.md` when REPLY is
  `approve run N`, and prints `approved run N: K cards`. With
  `--given`, REPLY must name exactly the cards under `## Run`. Exits 3
  when nothing was approved.
- `hub auto now` writes and prints `brief-now.md`: `Auto run N`, one
  line per card and `Questions for you: N`. Exits 3 with
  `the run is not approved; start nothing`.

## Commands for the user's screen

- `hub flag NAME` marks NAME's pane "needs you" and prints
  `flagged NAME`. `hub unflag NAME` clears it and prints
  `unflagged NAME`.
- `hub alive SESSION` prints an OpenCode session's activity, such as
  `42 messages, newest 14:05:09, last tool bash, working`. The last
  word is `working`, `finished` or `stopped`. Exits 1 with
  `no OpenCode session SESSION on this machine`.
- `hub go` shows the next hub in the sidebar and prints
  `showing NAME`. `hub gather` moves the hubs' workspaces, and the
  desk, to the top of the sidebar, cuts Herdr's agent list to the
  hubs and the sessions that wait on the user, and prints
  `at the top: IDS` with the workspace IDs. Both exit 3 with
  `no hub is running`.
- `hub desk` makes the `desk` workspace when it is missing, with
  `hub board` in its first tab and a shell in its second, and moves
  it below the kanban hubs. It prints `made desk: board, then shell`
  or `desk is open`.
