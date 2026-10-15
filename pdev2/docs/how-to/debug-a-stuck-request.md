# Debug a Stuck or Unanswered Request

Use this when an agent says its request is still waiting, you approved and
nothing was posted, or no popup or phone question appeared. The runner and
the phone question are explained in
[How a request runs gh once](../explanation/request-flow.md#the-runner),
and dismissed requests in the
[reference](../reference/requests.md#queue-inbox-and-marker).

## 1. Find the Record

The line `pde-gh-write` printed when it started waiting contains the
request ID. Without it, list the newest records:

```bash
ls -t "${XDG_STATE_HOME:-$HOME/.local/state}"/pde/pr-write/*.json | head
```

Show the fields that matter, with `ID` replaced:

```bash
jq '{state, created, updated, dismissed, runner_pid, moshi_pid, moshi_error,
  gh, exit_code, pane_id}' \
  "${XDG_STATE_HOME:-$HOME/.local/state}/pde/pr-write/ID.json"
```

`null` means the field is not saved. For `runner_pid` or `moshi_pid`, no
such process was saved, so there is nothing to check in step 3. For
`dismissed`, the request was not dismissed.

## 2. Read the State

| State | What it means | What to do |
|---|---|---|
| `pending`, `dismissed: true` | You dismissed it, so it no longer opens on its own | Press Ctrl+b and then `a` in Herdr, run `pde-pr-approve ID`, or have the agent rerun the command |
| `pending` | Nobody has answered | Check the runner and the phone question in step 3 |
| `approved` | Answered, but `gh` has not run | Check the runner and the `gh` path in step 3 |
| `declined` | Declined | Ask again with a change, or after four hours |
| `running` | A run stopped partway, and `gh` may already have posted | Check GitHub before asking again |
| `ran` | Done | Rerun the command to print what `gh` printed |

A record whose `updated` time is more than four hours old has expired. The
next identical command replaces it with a new waiting request.

## 3. Check the Runner and the Phone Question

Replace `RUNNER_PID` and `MOSHI_PID` with the numbers in `runner_pid` and
`moshi_pid` from step 1:

```bash
ps -o pid=,args= -p RUNNER_PID
ps -o pid=,args= -p MOSHI_PID
```

A running runner prints a line ending in `--pde-runner ID`, and a running
phone question one ending in `--moshi-ask ID`. No output means that program
is gone. For an agent in Herdr, a running phone question sends nothing to
the phone until the request is 30 seconds old.
If the record is `approved` and the file named by `gh` is missing,
the runner could not run it and left it approved for the next rerun.

If `moshi_error` is set, the last phone question ended without an
answer, and the waiting command printed `the phone question failed`.
`exit status 4` means `moshi-hook` could not reach Moshi: its service is
not running, it is not paired, the computer is offline, or the push was
dropped. Check with:

```bash
moshi-hook status
moshi-hook doctor
```

Then rerun the command to send a new question.

## 4. Rerun the Exact Command

Run the identical command from the same folder; the folder is part of the
request ID. For a waiting request, this starts a new runner and phone
question if the saved ones are gone, opens the popup again if the request
was dismissed, then waits up to 90 seconds again. For
an approved request, it runs `gh` with the `gh` it finds on `PATH`. For a
finished request, it prints the result.

## 5. If No Popup Appeared

The popup opens only when the agent runs inside Herdr, where `HERDR_ENV` is
`1`, and the `pde.approval` plugin is linked into Herdr. Check the plugin
with:

```bash
herdr plugin list --json | jq '.result.plugins[] | select(.plugin_id == "pde.approval")'
```

No output means the plugin is not linked. Link it while Herdr is running:

```bash
herdr plugin link ~/.local/share/pde/herdr-plugins/pde-approval
```

If the plugin is listed, check that the agent's shell still has
`HERDR_ENV`. Run this in that shell:

```bash
echo $HERDR_ENV
```

If it prints nothing, the shell lost it; put `HERDR_ENV=1` before the
command, as in `HERDR_ENV=1 pde-gh-write pr comment 12 --body "..."`.

If a notification starts with `Cannot open the approval popup:`, the
rest is Herdr's reason. If it names a workspace or says to close a
popup, go to step 6.

Record fields are listed in the [reference](../reference/requests.md#files).

## 6. If Another Popup Is Open

Herdr shows one popup at a time. An approval popup opens in the
workspace in front of you, and once you switch away you no longer see
it. Until you answer or close it, Herdr refuses every other popup, so
later requests appear nowhere.

Press Ctrl+b and then `a`. If an approval popup is open, Herdr moves to
its workspace and a notification says
`Approval waiting: answer the popup in NAME`.
Answer there; the popup then moves on to the next waiting request. A
new request shows the same notification instead of
`Answer the popup in NAME`.

If the notification says `Close the open Herdr popup or menu, then
press Ctrl+b a`, close Settings, Copy mode, or another plugin's popup,
or check the other workspaces for an unanswered approval popup. Then
press Ctrl+b and then `a` again.

Herdr's log shows each refused popup as `outcome="error"`:

```bash
grep 'method="plugin.pane.open"' ~/.config/herdr/herdr-server.log | tail -5
```

## 7. If Ctrl+b a Does Nothing

Herdr runs `pde-pr-approve --herdr-inbox` for Ctrl+b a and keeps what it
printed. Show the newest runs:

```bash
herdr plugin log list |
  jq -c '.result.logs[] | select(.action_id == "inbox") | {exit_code, stderr}' |
  tail -5
```

Exit 0 means the popup opened, or the inbox showed a notification. On
exit 1, `stderr` holds
`pde-pr-approve: cannot open the approval popup: REASON`. A
`pde-pr-approve: herdr notification show:` line, with either exit code,
means the notification could not be shown.

If `REASON` is `exec: "herdr": executable file not found in $PATH`, the
inbox found `herdr` neither on the `PATH` Herdr gave it nor next to
`pde-pr-approve`. Check that `~/.local/bin/herdr` exists, and see the
`PATH` of the Herdr server:

```bash
tr '\0' '\n' < /proc/$(pgrep -of 'herdr server')/environ | grep '^PATH='
```

For other reasons, see steps 5 and 6.
