# hub

A small command for the hub, the one Herdr session that other agent
sessions report to. It shows every session on one short table, sends
messages without losing them, wakes the hub when a session has news,
and marks sessions that wait on the user. The `herdr-hub` skill says
when to use each command.

## Install

`pde-installer install` builds and installs it on the client and full
profiles, at `~/.local/bin/hub`. `build.py` packs the `.py` files in
`src/hub/` into one file that runs with `/usr/bin/python3`, so a
machine needs only Python 3.10 or later.

## Commands

```text
hub [--machine LABEL] status --run DIR   sessions, then active cards
hub [--machine LABEL] send --run DIR NAME MESSAGE
hub next --run DIR [--timeout SECONDS]   wait for news, print one line
hub read --run DIR NAME [--note TEXT]    print, archive, log a report
hub alive SESSION                        working, finished or stopped
hub [--machine LABEL] flag NAME
hub [--machine LABEL] unflag NAME
hub handoff --run DIR NAME HANDOFF       start the next hub
hub brief --run DIR                      what waits on the user
hub auto ready --run DIR                 ready cards, for sorting
hub auto check --run DIR                 check the run's card list
hub auto approve --run DIR N REPLY       save the user's approval
hub auto now --run DIR                   one line per run card
hub --version
```

`DIR` is the hub's run folder. `--machine` passes a saved Herdr
machine to every `herdr` call, as `herdr --machine` does.

- `status` prints one row per named session, leaving out the hub's
  own: its state, its context size, its card key, whether its report
  is unread, whether a message waits for it,
  and its pane's label. Then it prints one row per active card, from
  the `card` command: the card's newest live session, its state, and
  whom it waits on. `you` means the user: the session is blocked, is
  marked "needs you", has an unanswered needs-you note on its card, or
  is idle and has never reported to this hub. `hub` means an idle
  session that reports to this hub. When `card` fails, one `cards:`
  line gives the reason. The context size comes from OpenCode's
  database for the session Herdr names, counted as `pde-context`
  counts it, such as `108k`. It is `-` with `--machine` or when the
  database has no count for the session.
- `send` sends the message at once when the session is `idle` or
  `done` and the user is not in its pane. Otherwise it keeps the
  message in `DIR/send-NAME.json`, and `hub next` sends it later. A
  second message while one waits exits 2. When Herdr gives up after
  taking the text, the message stays in `DIR/send-NAME.sending`, since
  it may have arrived. When Herdr says the prompt stalled and the
  screen shows the text pasted in the input box, `send` presses Enter
  once and counts the message sent when the session starts.
- `next` checks every 5 seconds and prints one line, then exits:
  `report NAME` for an unread `DIR/report-NAME.md`, `blocked NAME` the
  first time a session is seen blocked, `failed NAME: REASON` when
  Herdr refused a waiting message before taking it, or
  `unsure NAME: REASON` when it may have arrived. `timeout` means
  `--timeout` ran out. `HUB_POLL_SECONDS` changes how often it checks.
- `read` renames `DIR/report-NAME.md` to `DIR/report-NAME.HHMM.read`,
  with the time, and prints it. A second report from NAME read in the
  same minute becomes `report-NAME.HHMM-2.read`, so no archive is
  replaced. The last line is `unread:` and the names of the other
  unread reports, or `-`. `--note TEXT` adds `- HHMM TEXT` to
  `DIR/notes.md`. With no unread report from NAME it exits 2.
- `alive` prints one line on an OpenCode session of this machine,
  such as `412 messages, newest 14:03:22, last tool shell, working`.
  The last word is `finished` when its turn ended, `working` while a
  tool runs or a message changed in the last 2 minutes, and `stopped`
  otherwise. An unknown session exits 1, and `--machine` exits 2.
- `flag` makes the pane show "needs you" in place of its state:
  while `blocked` when it is blocked now, else while `idle` or `done`.
  `unflag` removes it.
- `handoff`, run in the old hub's pane, starts the next hub NAME in a
  new tab of the old hub's workspace, on the model of the last `--model
  '...'` in `DIR/rules.md`, and sends it the HANDOFF file. Before that
  prompt it writes the old hub's session review handoff in
  `~/.local/state/pde/handoffs/` and adds to HANDOFF how to close the
  old hub's tab and start that review. It exits 2 when the next hub's
  first reply, read from OpenCode's database, ran at another reasoning
  effort than the model's `#<effort>`, or came not at all, or when Herdr
  fails after the start. Its message then starts with `started NAME,
  but`, since the next hub runs and a second `hub handoff` would start
  another. It also exits 2, before starting anything, when it finds no
  model, no HANDOFF file, no Herdr pane, or no OpenCode session for the
  old hub.
- `brief` prints `DIR/reviews.md`, then every entry of
  `DIR/decisions.md` in full. It exits 2, printing nothing, when an
  entry lacks its question, two choices, `Recommended:` or
  `If no reply:`.
- `auto` runs the steps of an auto run, which the `auto-run` skill
  describes. DIR is the auto run's folder. `ready` writes the title,
  project, Context and Acceptance Criteria of every ready card to
  `DIR/ready.md` and prints it. `check` exits 2, naming each problem,
  when `DIR/run.md` lacks its `# Auto run N` title or a card under
  `## Run`, lists a card that is not ready or lists one twice, or
  leaves out a ready card. `approve` writes `DIR/approved.md`, with
  the time, REPLY and `run.md`, only when REPLY is `approve run N`,
  ignoring case, spaces, quotes and end punctuation, the list passes `check` and N is its number; otherwise it exits 2.
  `now` writes `DIR/brief-now.md` and prints it: one line per card
  under `## Run`, from the first line of `DIR/status-<key>-auto.md`,
  marked `(needs you)` while a `decisions.md` entry's title starts
  with the key, then the number of questions and each one's title. It
  exits 2 before the run is approved.

Exit codes are 0 when the command did its work, 1 when a `herdr` or
`card` call failed, with its message on standard error, and 2 when
refused.

## Files in the run folder

| File | Meaning |
| --- | --- |
| `report-NAME.md` | News from NAME the hub has not read. `hub read` renames it to `.HHMM.read`. |
| `notes.md` | The hub's log for the user. `hub read --note` adds a line to it. |
| `send-NAME.json` | A message that waits until NAME is free. |
| `send-NAME.sending` | A message being sent. One left behind means a send stopped part way, and the message may have arrived. |
| `send-NAME.failed` | A message Herdr refused before taking it, with its reason. |
| `next-seen.json` | The blocked states `hub next` already printed. |
| `rules.md` | The user's standing decisions, written by the hub. |
| `reviews.md` | Pull requests that wait for the user's review. |
| `decisions.md` | One `## <title>` entry per question for the user. `src/hub/brief.py` shows the form. |
| `ready.md` | An auto run's ready cards, written by `hub auto ready`. |
| `run.md` | The auto run's card list, written by the front hub. `src/hub/auto.py` shows the form. |
| `approved.md` | The approved list, with the time and the user's reply. |
| `brief-now.md` | One line per run card, written by `hub auto now`. |

## Tests

```sh
uv sync --locked
uv run --locked pytest
```

The tests build the packed file and run it against a fake `herdr` and
a fake OpenCode database, so they never reach a real session.
