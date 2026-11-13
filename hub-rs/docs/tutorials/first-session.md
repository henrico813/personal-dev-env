# Your first hub session

In this tutorial you act as a hub by hand. You start one worker,
send it a message, read its report and see which tab `hub` would
close. It takes about ten minutes.

A **hub** is an AI agent session that starts other sessions, called
**workers**, and keeps track of them. **Herdr** is the terminal app
that runs these sessions in tabs and tells `hub` what state each one
is in. `hub` is the command the hub runs to watch, message and tidy
its workers. A real hub runs these commands for you; doing them once
yourself shows what it does.

You need a shell in a Herdr pane on a machine set up with
pde-installer.

## 1. Install hub

`pde-installer install full` builds `hub` from `hub-rs/` and puts it
in `~/.local/bin/hub`. Check it:

```sh
hub --version
```

It prints `hub` and a version, such as `hub 0.8.0`.

## 2. Make a run folder

A **run folder** holds one hub's reports, messages and notes. Every
command in this tutorial reads it from `HUB_RUN`, so none of them
picks the run folder of a real hub on this machine:

```sh
mkdir -p /tmp/hub-demo
export HUB_RUN=/tmp/hub-demo
hub note "started the demo"
```

`hub note` adds a timed line to `notes.md` in the run folder and
prints the time, such as `noted 0856`.

## 3. Look before you wait

`hub` looks at every session in Herdr on this machine, not only the
ones this run folder knows. The first `hub next` in step 6 asks each
card's worker at 150k tokens or more to hand off to a new session.
It closes nothing; only `hub clean` closes tabs. Check first:

```sh
hub status
```

In the table, if any row's WHY ends in `hand off`, stop here:
`hub next` would ask that session to hand off.

## 4. Start a worker

Make a tab for the worker and start an OpenCode session in it named
`demo-docs`. `--env` gives the worker the same `HUB_RUN`:

```sh
P=$(herdr tab create --no-focus \
  --workspace "$HERDR_WORKSPACE_ID" \
  --label demo-docs --cwd /tmp/hub-demo \
  --env HUB_RUN=/tmp/hub-demo |
  jq -r .result.root_pane.pane_id)
herdr agent start demo-docs --kind opencode \
  --pane "$P" --timeout 90000
```

Then list the sessions:

```sh
hub status
```

The top lines describe the hub and the board, then a table has one
row per session. Your numbers and other rows differ:

```text
hub - -, watcher stopped
board 24/8, review 19
open 1: 0 working, 0 hub, 0 you,
  1 kept, 0 to-close, 0 to-park; 0 parked
unread 0

NAME       STATE  CTX  WHY
demo-docs  kept   -    yours
```

`hub - -` means this pane runs no agent, so it has no name or context
size. `watcher stopped` means no `hub next` runs for this folder. The
board line reads `board: REASON` when `card` is not set up. STATE
`kept` with WHY `yours` means the session never reported, so `hub`
treats it as yours and leaves it open.

## 5. Send it a message

Write the message to a file, then send it:

```sh
cat > /tmp/hub-demo/ask.md <<'EOF'
Write two short lines about yourself. Send them with
printf '%s\n' "line one" "line two" | hub report demo-docs
using your own lines, then end your turn.
EOF
hub send demo-docs --file /tmp/hub-demo/ask.md
```

It prints `sent demo-docs`. When the worker is busy, it prints
`held demo-docs: WHY` instead, and `hub next` sends it later.

## 6. Wait for the report

The worker runs `hub report demo-docs`, which prints
`report demo-docs written, 2 lines` in its pane. Wait for news:

```sh
hub next --timeout 300
```

It prints one line and exits:

```text
report demo-docs (hub -)
```

`(hub -)` is the hub's context size, unknown here. If it prints a line
about another session, such as `blocked docs-fix (hub -)` or
`to-close docs-fix: report read; run hub clean`, run it again.
`timeout (hub -)` means nothing came in 300 seconds.

`blocked demo-docs (hub -)` means OpenCode asks you to approve the
worker's `hub report` command. Approve it in the `demo-docs` tab,
then run `hub next` again.

## 7. Read the report

```sh
hub read demo-docs --note "read the demo report"
```

It prints the report, then `unread: -`, which lists any other unread
reports. It renames the report to
`report-demo-docs.HHMM.read` and adds your note to `notes.md`.

## 8. Tidy up

If you are in the `demo-docs` tab, switch back to your own tab first.
`hub` keeps open a session whose pane you are in, and the dry run
would print `kept demo-docs: user in pane`.

The worker reported and you read its report, so its tab can close:

```sh
hub clean --dry-run
```

Among its lines is:

```text
would close demo-docs: report read
```

If `would close demo-docs` is the only `would close` or `would park`
line, run `hub clean`. It prints `closed demo-docs: report read`.
Otherwise close the demo tab in Herdr yourself.

Last, read the log the hub keeps:

```sh
cat /tmp/hub-demo/notes.md
```

It has a date heading and one timed line for each note and close.

## Next steps

- [Steps for an agent](../how-to/agent-steps.md) lists what a worker
  and a hub run, line by line.
- [Commands](../reference/commands.md) lists every command, flag and
  exit code.
