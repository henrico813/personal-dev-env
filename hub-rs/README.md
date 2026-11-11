# hub in Rust

The `hub` command, rewritten in Rust. A hub is the Herdr session that
other sessions report to, and it runs `hub` to watch, message and
tidy them. The Python hub in `../hub/` stays the one agents and skills
use until this program replaces it.

pde-installer builds this folder on the Development tier and installs
it as `~/.local/bin/hub-next`, so you can try it beside the Python
`hub`. So far it has `next`, `send`, `read`, `report`, `inbox`,
`note`, `brief`, `handoff`, `flag`, `unflag`, `alive`, `go` and
`gather`.

```bash
make build   # target/release/hub
make check   # format, lint and test, as CI does
make test
```

Exit codes: 0 done, 1 an outside system such as Herdr failed, 2 wrong
usage, 3 not done, such as when no hub is running. Each failure prints
its reason.

## Differences from hub

- `hub-next --version` prints `hub 0.2.0`, since the binary already
  calls itself `hub`. The Python hub prints `hub 0.1.0`.
- `hub-next go` and `hub-next gather` exit 3, not 2, when no hub is
  running, so a script can tell this from a mistyped command.
- `hub-next flag` on a working session marks it on idle and done, so
  "needs you" shows once it stops. The Python hub marked the working
  state, which showed "needs you" while the session still worked.
- `next`, `send`, `read`, `report`, `inbox`, `note`, `brief` and
  `handoff` take the run folder from `--run DIR`, else from
  `HUB_RUN`. With neither, or with a folder that does not exist, they
  exit 2.
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
- `next` does not close or park finished sessions yet.
