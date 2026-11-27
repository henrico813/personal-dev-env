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
- While the wait is open, `status` shows a NAME it would list
  `to-park` or `to-close` as `kept`, WHY `waits on a job`, so `clean`
  leaves it open.

The prod starts with `hub watch: `, such as
`hub watch: your wait ended, since pid 4242 ended. Check the job and
go on.`

## Commands for the hub

### status

`hub status [--run DIR] [--json]` prints the hub's line, the board,
the count per state, the unread reports and one row per session:

```text
hub kanban-hub-18 131k, watch running, hub next running
board active 5 of 8, in-review 19 of 6
blocked 1, oldest 8 days
open 6: 1 working, 1 hub, 1 flagged,
  1 kept, 1 to-close, 1 to-park; 1 parked
unread 2: demo-007-impl docs-fix

NAME                  STATE     CTX   WHY
docs-fix              working   152k  1 held, hand off
demo-007-impl         hub       98k   unread report
demo-009-auto         flagged   12k   blocked
demo-012-auto         kept      77k   unpushed in /home/u/wt/demo-012
review-kanban-hub-16  to-close  60k   report read
demo-090-auto         to-park   104k  -
demo-081-auto         parked    -     since 11-10 22:41

no session: DEMO-KB-004 active
```

- The hub's line adds `: hand off now` after its context at 150k or
  more.
- `watch` is this machine's `hub watch`, the one `watch start`
  checks. `hub next` is a `next` waiting for this run folder. Either
  can read `stopped`, as in `watch stopped, hub next stopped`. A
  `next` ends after each line, so the hub starts a new one right after.
- The board line is `board: REASON` when `card` fails.
- The `blocked` line shows only when a card is in that
  stage, which waits on people outside the team and has no limit.
  `hub next` never counts these cards.
- STATE is one of `working`, `hub` (waits on the hub), `flagged`
  (waits on the user), `kept` (finished, but WHY says what keeps it
  open), `to-close`, `to-park` and `parked` (no tab, can reopen). A
  session whose todo, active, in-review or blocked card has an open
  question is `flagged`, with WHY `open question`.
- `no session:` names active and in-review cards that no session works
  on.
- `--json` prints the same facts as one JSON object.

### next

`hub next [--run DIR] [--timeout SECONDS] [--json]
[--start-reviews]` waits until something needs the hub, prints one
line, or two with a warning, and exits 0. While it waits, it sends held messages. At most
once a minute it reads the board and finds finished sessions. It
never closes or parks a session.

- `report NAME (hub 131k)`: NAME wrote a report.
- `missed report NAME (hub 131k)`: the report waited 10 minutes with
  no `next` line.
- `blocked NAME (hub 131k)`: NAME waits for the user.
- `not sent NAME: REASON (hub 131k)`: a held message did not arrive.
- `unsure NAME: REASON (hub 131k)`: a held message may have arrived.
- `timed message to N sessions: PATH (hub 131k)`: a message stored
  with `at` reached its time and is held for each session. PATH is
  the `--file` given to `at`.
- `board over its limit: active 10 of 8, in-review 18 of 6 (hub 131k)`:
  active or in-review holds more cards than its limit.
- `hand off now: hub at 152k`: the hub should hand off now.
- `to-close NAMES: WHY; run hub clean`, or the same with `to-park`:
  sessions that finished, listed once.
- `no hub handoff for OLD -> hub adopt OLD (hub 131k)`: an older hub
  of the run is open, and no `handoff` named it. Each hub sees this
  once for each such OLD.
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
`(hub -)` means the hub's context is unknown. When Herdr gives the
hub's pane no name, as it can after the hub's OpenCode restarts, a
second line follows every news line:
`hub pane has no name -> herdr agent rename PANE NAME`. `hub status`
prints the same line with its problems. The context is still read
by pane ID. `hub handoff`, `hub adopt` and `hub watch start` stop
with that line until the pane has a name again. `--json` prints one object with the same facts, the
second line as `hub_warning`:

```json
{"news":"report","name":"docs-fix","detail":null,"hub_context_tokens":131204,"hub_warning":null}
```

### send

`hub send [--run DIR] NAME (MESSAGE | --file PATH)` sends now when
the session can take it, and otherwise holds the message for `next`.
Each delivered message starts with `hub: `, so the worker tells it
from the user's own words.

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
is `board active 5 of 8, in-review 19 of 6; 2 questions`, or
`board: REASON`. Then come one line per pending pde-gh-write request,
oldest first, such as `approval A0016cd 1h12m left app #515 merge`,
then `reviews.md` and one section per open question, or
`Nothing waits on the user.` A pending request past pde-gh-write's
four-hour expiry does not show. A question's section starts
`Question KEY Q3, asked 2026-11-25: TITLE`, or `undated` in place of
the date, then its lines as `hub ask` saved them. Right
under the title, a `Check first: the user wrote in NAME TIME` line
names each session on the card where someone typed a message on or
after the day the question was asked, other than `hub send` messages,
watch prods, their older `From the hub` and `Your context is about`
forms, and the session's first prompt. NAME is the Herdr name,
or the session ID when Herdr does not list it. When
`card` cannot list the questions, the board line ends
`questions unread`, a `questions: REASON` section follows, and the
brief never says that nothing waits.
When cards are blocked, the board line is followed by
`blocked 2, oldest 8 days`, and the brief ends with a list
the user need not act on:

```text
Blocked (no action needed)
- DEMO-KB-003, 8 days
  Retry failed uploads
  platform team's review
```

### waiting

`hub waiting [--run DIR]` prints every open question for the user,
who reads it at the end of every hub message. An open question is an
item under Open Questions on a todo, active, in-review or blocked
card; draft, done and dropped cards are never listed. The first line
is `2 questions wait on you.`, then each question oldest first, by
the day it was asked, then card key and number, with undated items
last. Each starts `**Q3. TITLE (KEY, asked 2026-11-25)**`, or
`undated` in place of `asked DATE`, then its lines as `hub ask` saved
them, and a `Reply like `KEY Q3A`.` line, with the first question's
key and number, ends the list. With no open question it prints only
`No question waits on you.`

- The number is the item's Q number on its card. `card` writes it
  into the item and never gives it to another question on that card,
  even after the answer, so a reply key from an older message still
  names one question. Two cards can each have a `Q1`, so a reply key
  starts with the card key. `--run` is accepted and not used.
- It labels the front hub's sidebar row `hub busy, N for you` or
  `hub free, N for you`, or plain `hub busy` and `hub free` when
  nothing waits. When that fails, a last line starts
  `Sidebar count not set:`.
- When `card` cannot list the questions, it prints only
  `Questions could not be read: REASON` and changes nothing.

### ask

`hub ask KEY FILE` puts the question in FILE on card KEY. It adds the
question as one item at the end of the card's Open Questions, where
`card` gives it the next Q number and today's date, so `hub brief`,
`hub waiting` and `hub board` show it. A card can hold several open
questions. Prints `KEY asks 'TITLE'`. FILE starts with `## TITLE`,
then the question, two or more `- ` choices, a `Recommended:` line,
an `If no reply:` line and an optional `Depends on:` line, as
herdr-hub's Questions for the user shows.

- Exits 3 with `the question must start with a ## title line`, with
  `question 'TITLE': PROBLEMS` when FILE lacks the question, two or
  more `- ` choices, `Recommended:` or `If no reply:`, and with
  `KEY is in STAGE; the hub lists questions only on todo, active,
  in-review or blocked cards` for a draft, done or dropped card, and
  with `KEY already asks 'TITLE' as QN` when an open question on the
  card has the same title. None writes to the card.

### answer

`hub answer KEY N ANSWER` adds open question QN of card KEY to its
Decisions as `Q3 asked 2026-11-25: TITLE`, or `Q3: TITLE` for an
undated item, then its lines and `Answer: ANSWER` last. Then it
removes item QN from Open Questions. N is `3`, `Q3` or `q3`. Prints
`KEY Q3 answered 'TITLE': ANSWER`.

- Exits 3, changing nothing, with
  `the answer must be one line of text` when ANSWER is empty or has a
  line break, and with `KEY has no open question QN`.
- Needs no Herdr pane: `card` logs the removal from Open Questions
  by `hub`.
- When the second read or the removal fails, or QN changed since the
  hub read the card, the error says the answer was added, and the
  question stays open. A hand edit can give a hand-typed item a new
  number, so find the question in `hub waiting`, then run
  `card edit KEY "Open Questions" Q4 --remove` with its number in
  place of `Q4`. Never run `hub answer` for it again, since that adds
  a second answer.

### requests

`hub requests [--json]` reads every pde-gh-write request file and
prints one row per request, newest change first: each live session's
requests changed in the last day, and every open request whatever
its session. SESSION is `-` when no live session filed it, and AGE
counts from the last change.

```text
REF      SESSION           STATE    AGE    PULL REQUEST
Aaa0004  demo-008-impl     pending  5m     app #20 merge
Aaa0002  demo-007-impl     expired  5h00m  app #20 merge
```

STATE is the file's `state`, or `expired` for a pending or approved
request four hours after its last change, when pde-gh-write no
longer runs it. Only `pending` waits on the user. A `running` request
that old is a run that died; it shows only as a live session's
request. With no rows it prints
`No requests from live sessions in the last day.` A last line
`requests: N files did not parse` names unreadable files.

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
that no `handoff` named, or an auto run's hub, such as `auto-hub-2`,
once its run is over. It writes OLD's review handoff and records a
handoff, so `clean` closes OLD once it is idle. For an auto hub, the
handoff also names its run, the one `~/.local/state/herdr-hub.*`
folder whose event log names OLD as its hub. When none or more than
one does, it names no run, and `clean` closes only OLD's tab. Run it
in the hub's own pane. Prints `adopted OLD; hub clean closes its tab
once it is idle, then hub next --start-reviews starts review-OLD`.

- Exits 3 with `OLD is not an auto hub or an older hub left without
  hub handoff`.
- Exits 3 with `OLD has sent no end brief to this run folder yet`
  for an auto hub with no `report-OLD` file, read or unread, in the
  run folder.

`handoff` and `adopt` also move the hubs to the top of the sidebar,
as `gather` does. When that fails, they add
`could not put the hubs on top: REASON; hub gather tries again`.

### launch

`hub launch [--run DIR] NAME HANDOFF --model MODEL [--workspace LABEL]`
starts a worker named NAME on MODEL in the current folder, prompts it
to read HANDOFF, and records the start. A card key at the start of
NAME, such as `demo-007-impl`, makes it that card's worker. The
worker opens in a new tab of the workspace whose label starts with
the key, else in a new workspace such as `demo-007 impl`. Work
with no card opens in the hub's workspace, so run it in the hub's
own pane. `--workspace LABEL` opens it in that workspace, and makes
the workspace if none has that label. A todo card moves to active
first, so `card` checks the active limit before anything starts. After
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
  refuses the move to active, such as past the active limit.
- A failure after the move begins `moved KEY to active, but REASON`.
  One after the start begins `started NAME` and lists each later step
  that finished, such as `started NAME, sent the handoff prompt,
  added the card note, but REASON`, so the hub does the rest by hand.
  Running it again after a start refuses, since NAME already runs.

`hub launch [--run DIR] NAME HANDOFF --successor` is how a worker
starts its own successor when `hub watch` asks it to hand off. NAME
opens in a new tab of the caller's workspace, on the model and effort
of the caller's newest reply, and the caller's "needs you" mark goes.
The caller is the agent whose OpenCode session runs the command, from
`OPENCODE_SESSION_ID`. The card steps and `notes.md` line are the
same. `--successor` takes no `--model` or `--workspace`.

- Exits 3 before it changes anything outside OpenCode, or when no
  live agent runs the caller's session or that session has no reply
  to take its model from.

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

The watch also asks each idle OpenCode session on this machine at
150k context or more to hand off, with or without a hub. It never
asks a hub, or a session in the `desk` workspace. The ask starts
`hub watch: your context is about 152k` and names
`hub launch --successor`. It comes again at each 20k more, such as
170k and 190k, and from 190k the session is also marked "needs you".
`hub/handoff-asks.json` keeps the highest context asked for each
session, so a restarted watch does not ask again at the same step.

The watch also reads `~/.local/state/pde/update.json`, which
`pde-installer update` writes. For each new `failed` or `skipped`
result it sends the phone one notification, titled
`pde-installer update failed` or `pde-installer update skipped`, with
the reason. The same result again sends nothing, even after a restart,
since the watch keeps the last result in `hub/update-seen.json`.

The watch also does what `hub restore-sidebar` does in each check.
Every 5 minutes it sets the missing `go_to` tags and the desk row on
each saved machine too, for a machine whose Herdr runs without the
`pde.hub` plugin, and clears none there. The user's Herdr lists those
machines' agents through its own filter.

### clean

`hub clean [--run DIR] [--dry-run]` closes the tab of each `to-close`
session and parks each `to-park` session. It acts only on tabs that
hold a session of DIR's run: those `hub launch` started from DIR,
those that reported to DIR, and the old hubs DIR handed off from. A
tab with none of them stays open, except in a finished auto run's
workspace, as the next paragraph says. A finished `to-close` session of
another run closes with a tab it shares with DIR's sessions, and the
`closed` line names it. Any other agent there keeps the tab. Another
run's sessions get no `parked` line, and a `kept` line only when the
planned close of a tab they share stops. Only `clean` and `park`
close tabs.

When an auto hub that DIR adopted is `to-close`, every tab in its
workspace, and in the workspace of each session its run's `notes.md`
says it started, may change too. A workspace that holds this hub's
own pane or another hub that stays open is left out, and so is the
user's `desk` workspace. Each agent there is judged by its own
state. A tab of shells that run nothing closes. Herdr closes the
workspace with its last tab. A pane that keeps one of those tabs
open gets a `kept` line. Without `HERDR_PANE_ID`, or when Herdr or
the run folder cannot be read, no workspace closes this way, and only
the old hub's tab closes.

- `closed NAME: WHY` and `parked NAME: KEY in-review`.
- `closed pane PANE-ID: run over` for a tab of shells that run
  nothing, naming each pane, such as
  `closed pane P1, pane P2: run over`, and
  `closed workspace LABEL: run over` once the workspace closed.
- `forgot NAME`: a parked session that is live again.
- `kept NAME: WHY` for each kept session, and for a finished one
  whose tab also holds other work, with `its tab holds other work`.
  In a closing workspace, an agent that is not finished gets one too,
  such as `kept NAME: working`.
- `kept pane PANE-ID: WHY` for a pane that keeps a tab of a closing
  workspace open: `shell in use`, `agent with no name` or `focused`.
- `kept workspaces of OLD: WHY` when clean cannot tell which
  workspaces of OLD's run may close, such as
  `this hub's pane is unknown`.
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
- The record keeps the session NAME was launched as, from the
  card's Log, even when a nested `opencode run` made Herdr show
  another. `clean` parks the same way.

### board

`hub board` opens on its first screen, sized for a 40x16 phone
screen. A red `!` row names each fault on top, then four blocks
follow: WAITING ON YOU, SINCE YOU LEFT, ACTIVE and IN REVIEW. Each
block's heading ends in the key of its page, such as `[3]`. Below
100 columns the blocks stack and show 3, 1, 2 and 0 rows; from 100
columns WAITING ON YOU and SINCE YOU LEFT sit left of ACTIVE and
IN REVIEW, and show 6, 7, 2 and 6 rows. A block with more rows ends
in a dim fold row that sums up the rest, such as
`+ run 24 hub and 2 old questions`, `+ 1 report`,
`+ 3 more, 1 at work` or `+ 2 more`. WAITING ON YOU's fold row names
an auto run's hub by its run, and calls a question not asked today
old. The top line says how long the user was away, then the
board's status, such as `away 1h29m · read 10:42 0.6s`.

- A fault is `hub watcher stopped` while a hub runs and no
  `hub watch` does, `auto run N stalled since ...` for a run
  approved in the last 24 hours that has stalled, as `hub watch`
  tells the phone, or a source the read could not get.
- WAITING ON YOU shows one row per card that waits on the user,
  with its open questions and waiting sessions on that one row. Its
  tag is `answer` when the card has an open question, and `see tab`
  when only a worker waits in its tab. The questions are the ones
  `hub waiting` lists. Rows with a question asked today come first,
  then `see tab` rows, then older questions, newest first. A
  waiting session with no card key has its own row,
  `Hub of auto run 24` for an auto run's hub, else its session name.
  The heading counts the rows. With no hub running, the block shows
  `no hub running`.
- SINCE YOU LEFT shows one row per card that changed after the time
  the board last saved, newest first, such as
  ` 10:40 DEMO-KB-001 Faster search  report`. The row ends in its
  newest event's word: `report` for a worker report, `new worker`
  for a launch and `finished` for a parked worker. Any other event,
  such as a closed session or the hub's own note, shows
  its own text. Events that name an auto run's hub share a row named
  `Hub of auto run 24`, and other events with no card share one
  `hub` row.
  The heading reads `SINCE YOU LEFT AT 19:07  7 cards`, or
  `SINCE YOU LEFT  newest` when no board has saved a time yet. Once
  a time is saved, the block also reads runs whose hub has ended,
  so what a hub did before it ended still shows.
- ACTIVE shows the active cards: those with a worker at work, then
  those with no worker, then the idle ones. Its heading counts them
  against the active limit from `card board`, then the cards at
  work, such as `23, max 8 · 2 working`. The count turns yellow over
  the limit, and `max` is left out when `card board` gave no limits.
- From 100 columns each WAITING ON YOU row has a dim line below it
  with its question or why its tab waits. Each ACTIVE row has one
  with its card's newest event since your visit, such as
  `19:43: new worker`. A row with neither has no such line.
- IN REVIEW counts the cards in review the same way, then the pull
  requests with a failed check, such as `7, max 6 · 1 PR fails`.
  Below 100 columns it shows only its heading.
- A row starts with its status word: `you` waits on the user, `run`
  a worker is at work on it, `idle` it has a worker and none is at
  work, `none` it has no worker and `rev` to review. Page 5's pull
  request rows have no status word and end in their check word. A
  worker is at work while Herdr shows it working, or for 30 minutes
  after its `status-NAME.md` in a live hub's run folder last changed.
  A card whose worker waits on the user is not at work, so ACTIVE and
  page 3 list it with the idle cards. The first screen's
  WAITING ON YOU and ACTIVE rows leave the status word out, and
  ACTIVE rows also the project label.
- A row about a card starts with its card key, the ID at the start
  of the card's name, such as `DEMO-KB-001`. An event and a waiting
  session have no card field, so the board takes the card key from
  the first word that holds one, such as `DEMO-KB-001` from the
  session name `demo-kb-001-build`.
- Keys 1 to 5 open the pages, each under a breadcrumb such as
  `< 3 ACTIVE`: 1 WAITING ON YOU, 2 SINCE YOU LEFT, 3 ACTIVE,
  4 SESSIONS AND MACHINE and 5 IN REVIEW AND PULL REQUESTS. Tab steps
  through the pages and back to the first screen. A tap on the
  breadcrumb, Esc or q goes back, and the first screen chooses again
  the row it had. ? opens the help of the first screen or of the open
  page. r reads again, and q quits from the first screen. Ctrl-C
  quits from anywhere. The board also reads again every 10 seconds.
- The first screen picks rows as pages 1, 3 and 5 do: j, k, the
  arrow and page keys or the wheel move the choice, drawn reversed
  across the row, and Enter, space or a tap acts on it. It starts on
  the first row, and a column scrolls to keep the choice in view when
  its rows do not fit, such as below fault rows. A WAITING ON YOU,
  ACTIVE or IN REVIEW row, or its dim line, opens its details. A
  block's heading opens its page, a fault row page 4 and a SINCE YOU
  LEFT row page 2. A fold row opens its block's page with every row
  the fold row sums up shown, so on page 3 IDLE opens. Those rows stay
  shown when the page opens again, as after `+ N more` on a page.
  Blank lines and a dim row such as `nothing new` cannot be chosen.
  From 100 columns the choice runs down the left column, then the
  right. A new read or visit time keeps the chosen row when it is
  still there.
- Below 100 columns the first screen's bottom line reads
  `4 sessions and machine` with `? help` at its right end. From 100
  columns it lists each page's key, help and quit:
  `1 waiting  2 since  3 active  4 sessions and machine  5 review  ? help  q quit`.
  On pages 1 to 5 the bottom line reads `esc back  ? help` and the
  status. On a details page and a help page it reads
  `tap or q to go back`.
- `NO_COLOR` turns colors off. Each color means one thing: cyan at
  work, magenta waits on you, green ready for review, passing,
  finished or ok, yellow held up, such as Blocked, `none` or a
  question's age, and red a problem. So the `answer` and `see tab`
  tags are magenta, `fails` is red and `pass` green, and the event
  words `new worker` and `finished` are cyan and green. `idle`, fold
  marks, event times and the Idle and Draft counts are dim. Group
  names are bold in the default color, and their counts take the
  color. Page 4's session rows keep the default color.
- Pages 1, 3 and 5 list cards by group. An open group shows four
  rows, six in WAITING ON YOU and IN REVIEW, then `+ N more`. Page 1
  has no group heading. A tap on a heading folds or opens it, and a
  tap on a row opens its details. j, k, the arrow and page keys or
  the wheel move the choice, and Enter or space acts on it.
- Page 1 lists the WAITING ON YOU rows, each with a line below.
  For a question that line gives the oldest question's age, how many
  more there are and the newest question, such as
  `16 days old, +3 questions: Who updates the docs?`; for a tab it
  says why the worker waits. With a card in review, the page ends in
  `then: review N cards`, which opens page 5. The rows come from
  every hub on this machine that Herdr lists: each
  `~/.local/state/herdr-hub.*` folder whose hub `hub watch start`
  named, auto runs included. The board ignores `HUB_RUN`, and the
  hidden `--run DIR` shows one run's sessions only.
- Page 2 lists the SINCE YOU LEFT cards, such as
  `> DEMO-KB-001 Faster search  report`. A tap on a row shows its
  events, one line each with its time and word, such as
  `   10:40 report`, and the row then ends in its event count. Another
  tap hides them. j, k, the page keys or the wheel scroll the page.
- Page 3 lists WORKING, NO WORKER, IDLE, BLOCKED, UP NEXT and DRAFT,
  and its breadcrumb carries the ACTIVE counts. BLOCKED and DRAFT
  start folded, and BLOCKED shows only when it holds a card. IDLE is
  folded below 100 columns and open from 100, and a tap folds or
  opens it until the width crosses 100 again.
- Page 4 shows the faults as red `!` rows, or a green
  `hub watcher ok` row when there is none. Then come the file-watch
  slots in use (`WATCH`, with `high` from 90%), free disk (`DISK`)
  and the sessions, grouped as `hub clean` decides: NEEDS YOU,
  AT WORK OR WAITING ON THE HUB, then one fold row for the rest,
  such as
  `> KEPT 11 · SAFE TO CLOSE 3 · PARKED 18`. When it fits, the
  AT WORK OR WAITING ON THE HUB heading also counts the sessions at
  work, such as `AT WORK OR WAITING ON THE HUB 2 · 1 working`; at
  40 columns it has no room. A tap on the fold row
  shows or hides those three groups. Each session row shows the
  name, a context bar full at 150K tokens, the context in thousands
  and why the session is in its group, such as `unpushed review`. A
  why too long for the row goes on the line below it. j, k, the
  arrow and page keys move the choice. A tap on a row only chooses
  it; the bar at the bottom then reads `-> jump to NAME`, and a tap
  on it shows that session in Herdr. Enter also jumps, but only
  after 0.7 seconds with no input, so a stray Enter after a tap does
  not jump. A failed jump shows `jump failed: REASON` on the bottom
  line.
- Page 5 lists IN REVIEW, each card with its pull request's line
  below it, such as `Demo #301 fails`, then OTHER PULL REQUESTS,
  failing first, such as `Demo #60 DEMO-KB-002 Faster board` tagged
  `fails`, `running` or `pass`. A pull request's label is the
  project of a card the read got whose `prs:` link names the same
  repository, else the repository name after `/`, such as `app`.
  Passing pull requests of no card share one row, such as
  `+ 2 pass, no card: #48 #52`. The pull requests come only from the
  `prs` tool's cache, `~/.cache/prs/list.json`. One belongs to a card
  only when the card's `prs:` link names the same repository and
  number. The read gets a card's note only when a session, a parked
  record or a report names the card. So a card that none of them
  names shows no pull request line, and its pull request is listed
  under OTHER PULL REQUESTS. The IN REVIEW heading adds its limit,
  such as `, max 6`, and the OTHER PULL REQUESTS heading its failing
  count, such as ` · 1 failing`.
- A card's details page lists its open questions under the title,
  and each worker's status line with its age, such as
  `api-build 12m ago: tests pass`. A waiting session's page adds its
  own status line. A question's page shows `KEY Q3, asked DATE` and
  how many days ago it was asked. A details page scrolls with j, k,
  the page keys or the wheel, and any tap, Enter, space, q or Esc
  goes back.
- c, from the first screen or any page, lists the SAFE TO CLOSE
  sessions, each marked `close` or `park`, and ends in `+ N more`
  when they do not fit. The list does not change while it is
  open. y, once the list has been open 1 second and no key came
  for 0.7 seconds, closes and parks the listed sessions,
  from any hub, and shows its output, with a `kept` line for any
  it now keeps. Any other key or a tap cancels. With nothing
  to close or no hub running, c says so and changes nothing.
- A visit is one stretch of the user using the board, ended by
  10 quiet minutes with no key or tap. The board saves the time
  of the user's latest key or tap in
  `~/.local/state/pde/hub/board-seen`, so the next board's
  SINCE YOU LEFT lists what came after that time. Taps while the
  user reads do not start a new visit. The live board writes the
  file at most once a minute, and once more as it quits. `--print`,
  `--dump` and `--control` read that time but never write it, so an
  agent's look at the board does not hide news from the user.
- The live board saves each read in
  `~/.local/state/pde/hub/board-snapshot.json`. A new board draws
  the saved read at once, with `saved HH:MM · reading` on top,
  until its own first read lands. `--print`, `--dump` and
  `--control` neither save nor load it.
- `--print` prints one screen as text, and `--dump` prints its
  elements as one JSON line. Both draw a new board, starting on
  the first screen, at `--size`, default 40x16.
- The first screen's element ids are `status`, `fault:N`, and for
  each block `block:SLUG` on its heading, `SLUG:N` on its rows and
  `more:SLUG` on its fold row, where SLUG is `waiting`, `since`,
  `active` or `review`. The chosen row is `chosen`, and a tap on a
  row does what Enter on it does. The bottom line holds `open:SLUG`,
  with `machine` for page 4, and `help`. A row the fold row sums up
  comes hidden, with its page's key as the step that shows it, such
  as `press 3`. A row scrolled off comes hidden with `press pageup` or
  `press pagedown`, as on the list pages.
- `--listen` shows the live board and also takes steps from
  `board.sock` in `~/.local/state/pde/hub`. From another
  terminal, `hub board --send 'STEP'` does one step on it and
  prints the screen that follows as one JSON line. It exits 2
  when the board cannot follow the step, and 3 when no board
  listens. A step an agent sends and a window resize do not count
  as the user's input, so they do not move the saved time. The
  socket works on Linux and macOS, not Windows.
- A step is `tap ID`, `tap COLUMN ROW`, `press KEY`, `wheel up` or
  `wheel down`. KEY is `enter`, `esc`, `space`, `tab`, `up`,
  `down`, `pageup`, `pagedown`, `ctrl-c` or one character, such as
  `q`. The hidden `--control`, which tests use, takes the same
  steps but never jumps or cleans.

"Screens agents can see and tap" in the [README](../../README.md)
describes the element JSON and the rules every such screen follows.
### auto

The steps of an auto run, in order. Each takes `--run DIR`.

- `hub auto todo` writes the todo cards to `todo.md` and prints
  them, or `No todo cards.`
- `hub auto check` checks `run.md` and prints
  `run N: K cards to run`. Exits 3 with one line per problem.
- `hub auto approve N REPLY` saves `approved.md` when REPLY is
  `approve run N`, and prints `approved run N: K cards`. With
  `--given`, REPLY must name exactly the cards under `## Run`. Exits 3
  when nothing was approved.
- `hub auto now` writes and prints `brief-now.md`: `Auto run N`, one
  line per card and `Questions for you: N`, then
  `- KEY Q3, asked DATE TITLE` for each open question on a card under
  `## Run`. A card with an open
  question gets `(needs you)`. Exits 3 with
  `the run is not approved; start nothing`.

## Commands for the user's screen

- `hub flag NAME` marks NAME's pane "needs you" and prints
  `flagged NAME`. `hub unflag NAME` clears it and prints
  `unflagged NAME`. Without `--machine`, each then sets the agent
  list as `hub restore-sidebar` does, and adds
  `could not set the go to list: REASON` when that fails.
- `hub alive SESSION` prints an OpenCode session's activity, such as
  `42 messages, newest 14:05:09, last tool bash, working`. The last
  word is `working`, `finished` or `stopped`. Exits 1 with
  `no OpenCode session SESSION on this machine`.
- `hub go` shows the next hub in the sidebar and prints
  `showing NAME`. `hub gather` moves the hubs' workspaces, and the
  desk, to the top of the sidebar, sets Herdr's agent list as
  `hub restore-sidebar` does, and prints
  `at the top: IDS` with the workspace IDs. Both exit 3 with
  `no hub is running`.
- `hub desk` makes the `desk` workspace when it is missing, with
  `hub board` in its first tab and a shell in its second, and moves
  it below the kanban hubs. It prints `made desk: board, then shell`
  or `desk is open`.
- `hub desk add NAME` moves NAME's pane into the desk as its last
  tab and prints `NAME is on the desk`. `hub desk rm NAME` moves it
  into a new workspace named NAME and prints `NAME is off the desk`.
  `hub desk move NAME N` makes it the Nth agent on the desk and
  prints `NAME is at place N on the desk`. Each exits 3 with
  `no desk is open; hub desk makes one`, and `rm` and `move` with
  `NAME is not on the desk`. `move` exits 2 with
  `the place must be 1 to COUNT`.
- `hub restore-sidebar [--start-board]` sets the agent list, headed
  `go to`, on this machine: the front hub, the desk's board and
  agents in tab order, then only the sessions that wait on the user.
  The front hub is the live agent that last ran `hub next` or
  `hub watch start` in a run folder without `approved.md`, when it
  has no name or a hub name other than an auto hub's. Else it is the
  live `kanban-hub-N` with the highest N. It moves no workspace. With
  `--start-board`, it also runs `hub board` in the desk's first pane
  when that pane's shell runs nothing. It prints
  `changed: NAMES` with each pane whose tag it changed and `desk`, or
  `nothing changed`, then `started the board in the desk` or
  `could not start the board: REASON`. The `pde.hub` Herdr plugin
  runs it with `--start-board` when Herdr starts, and without it each
  time Herdr detects an agent or moves a pane or tab.
