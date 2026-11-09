# hub

A small command for the hub, the one Herdr session that other agent
sessions report to. It shows every session on one short table. The
`herdr-hub` skill says when to use each command.

## Build

`build.py` packs the `.py` files in `src/hub/` into one file that runs
with `/usr/bin/python3`, so a machine needs only Python 3.10 or later:

```sh
python3 build.py ~/.local/bin/hub
```

## Commands

```text
hub [--machine LABEL] status --run DIR   sessions, then active cards
hub --version
```

`DIR` is the hub's run folder. `--machine` passes a saved Herdr
machine to every `herdr` call, as `herdr --machine` does.

- `status` prints one row per named session, leaving out the hub's
  own: its state, the context size from the pane's footer, its card
  key, whether its report is unread, and its pane's label. Then it
  prints one row per active card, from the `card` command: the card's
  newest live session, its state, and whom it waits on. `you` means
  the user: the session is blocked, is marked "needs you", has an
  unanswered needs-you note on its card, or is idle and has never
  reported to this hub. `hub` means an idle session that reports to
  this hub. When `card` fails, one `cards:` line gives the reason.

Exit codes are 0 when the command did its work, and 1 when a `herdr`
call failed, with Herdr's message on standard error.

## Files in the run folder

| File | Meaning |
| --- | --- |
| `report-NAME.md` | News from NAME the hub has not read. The hub renames it to `.read`. |

## Tests

```sh
uv sync --locked
uv run --locked pytest
```

The tests build the packed file and run it against a fake `herdr`, so
they never reach a real session.
