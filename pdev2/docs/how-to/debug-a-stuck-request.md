# Debug a Stuck or Unanswered Request

Use this when an agent reports a waiting request that never ran, or a popup
or phone question never appeared.

## 1. Find the Record

The waiting line printed by `pde-gh-write` contains the request ID. Without
it, list the newest records:

```bash
ls -t "${XDG_STATE_HOME:-$HOME/.local/state}"/pde/pr-write/*.json | head
```

Read the fields that matter:

```bash
jq '{state, created, updated, dismissed, runner_pid, moshi_pid, gh, exit_code, pane_id}' \
  "${XDG_STATE_HOME:-$HOME/.local/state}/pde/pr-write/ID.json"
```

## 2. Read the State

| State | What it means | What to do |
|---|---|---|
| `pending`, `dismissed: true` | It no longer opens on its own | Press `prefix+a` in Herdr, or run `pde-pr-approve ID` |
| `pending` | Nobody has answered | Check the runner and ask in step 3 |
| `approved` | Answered, but `gh` has not run | Check the runner and the `gh` path in step 3 |
| `declined` | Declined | Request again with a change, or after four hours |
| `running` | A run died, and `gh` may already have posted | Check GitHub before requesting again |
| `ran` | Done | Rerun to print the saved output |

A record whose `updated` time is more than four hours old has expired. The
next identical command replaces it with a new pending request.

## 3. Check the Runner and the Phone Ask

```bash
ps -o pid,args -p RUNNER_PID
ps -o pid,args -p MOSHI_PID
```

The runner shows `--pde-runner ID` and the ask shows `--moshi-ask ID`. No
output means the process is gone. If the record is approved but `gh` is
missing or not executable, the runner left it approved for a rerun.

## 4. Rerun the Exact Command

Run the identical command from the same directory; the directory is part of
the request ID. For a pending request this starts a new runner and phone ask
if the saved ones are gone, then waits up to 90 seconds again. For an
approved request it runs `gh` with the `gh` found on `PATH`. For a finished
request it prints the outcome.

## 5. If No Popup Appeared

The popup opens only when the writer runs inside Herdr, with `HERDR_ENV=1`,
and the `pde.approval` plugin is linked. Check with:

```bash
herdr plugin list --json | jq '.result.plugins[] | select(.plugin_id == "pde.approval")'
```

Record fields are listed in the [reference](../reference/requests.md#files).
