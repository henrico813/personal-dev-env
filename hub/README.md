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
hub [--machine LABEL] flag NAME
hub [--machine LABEL] unflag NAME
hub handoff --run DIR NAME HANDOFF       start the next hub
hub waiting --run DIR                    what waits on the user
hub --version
```

`DIR` is the hub's run folder. `--machine` passes a saved Herdr
machine to every `herdr` call, as `herdr --machine` does.

- `status` prints one row per named session, leaving out the hub's
  own: its state, the context size from the pane's footer, its card
  key, whether its report is unread, whether a message waits for it,
  and its pane's label. Then it prints one row per active card, from
  the `card` command: the card's newest live session, its state, and
  whom it waits on. `you` means the user: the session is blocked, is
  marked "needs you", has an unanswered needs-you note on its card, or
  is idle and has never reported to this hub. `hub` means an idle
  session that reports to this hub. When `card` fails, one `cards:`
  line gives the reason.
- `send` sends the message at once when the session is `idle` or
  `done` and the user is not in its pane. Otherwise it keeps the
  message in `DIR/send-NAME.json`, and `hub next` sends it later. A
  second message while one waits exits 2. When Herdr gives up after
  taking the text, the message stays in `DIR/send-NAME.sending`, since
  it may have arrived.
- `next` checks every 5 seconds and prints one line, then exits:
  `report NAME` for an unread `DIR/report-NAME.md`, `blocked NAME` the
  first time a session is seen blocked, `failed NAME: REASON` when
  Herdr refused a waiting message before taking it, or
  `unsure NAME: REASON` when it may have arrived. `timeout` means
  `--timeout` ran out. `HUB_POLL_SECONDS` changes how often it checks.
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
- `waiting` prints `DIR/reviews.md`, then every entry of
  `DIR/decisions.md` in full. It exits 2, printing nothing, when an
  entry lacks its question, two choices, `Recommended:` or
  `If no reply:`.

Exit codes are 0 when the command did its work, 1 when a `herdr` call
failed, with Herdr's message on standard error, and 2 when refused.

## Files in the run folder

| File | Meaning |
| --- | --- |
| `report-NAME.md` | News from NAME the hub has not read. The hub renames it to `.read`. |
| `send-NAME.json` | A message that waits until NAME is free. |
| `send-NAME.sending` | A message being sent. One left behind means a send stopped part way, and the message may have arrived. |
| `send-NAME.failed` | A message Herdr refused before taking it, with its reason. |
| `next-seen.json` | The blocked states `hub next` already printed. |
| `rules.md` | The user's standing decisions, written by the hub. |
| `reviews.md` | Pull requests that wait for the user's review. |
| `decisions.md` | One `## <title>` entry per question for the user. `src/hub/waiting.py` shows the form. |

## Tests

```sh
uv sync --locked
uv run --locked pytest
```

The tests build the packed file and run it against a fake `herdr` and
a fake OpenCode database, so they never reach a real session.
