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
jq '{state, created, updated, dismissed, runner_pid, moshi_pid, gh, exit_code, pane_id}' \
  "${XDG_STATE_HOME:-$HOME/.local/state}/pde/pr-write/ID.json"
```

`null` means the field is not saved. For `runner_pid` or `moshi_pid`, no
such process was saved, so there is nothing to check in step 3. For
`dismissed`, the request was not dismissed.

## 2. Read the State

| State | What it means | What to do |
|---|---|---|
| `pending`, `dismissed: true` | You dismissed it, so it no longer opens on its own | Press Ctrl+b and then `a` in Herdr, or run `pde-pr-approve ID` |
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
is gone. If the record is `approved` and the file named by `gh` is missing,
the runner could not run it and left it approved for the next rerun.

## 4. Rerun the Exact Command

Run the identical command from the same folder; the folder is part of the
request ID. For a waiting request, this starts a new runner and phone
question if the saved ones are gone, then waits up to 90 seconds again. For
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

Record fields are listed in the [reference](../reference/requests.md#files).
