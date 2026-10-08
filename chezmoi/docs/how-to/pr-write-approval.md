# Approve a pull-request write

## From the popup

Run the requested command through `pde-gh-write`. Read the exact request in
the popup. Press `y` to approve or any other key to decline. The wrapper waits
up to 90 seconds and then runs `gh` only after approval.

## From a terminal

When no tmux popup is available, run:

```bash
pde-pr-approve REQUEST_ID
```

Press `y` after reviewing the request, then rerun the identical
`pde-gh-write` command. The agent sees exit 3 while waiting or timing out,
exit 4 when you decline, and the underlying `gh` result after approval.

## After reconnecting

Attach to tmux normally. The `client-attached` hook looks for pending requests
and restores at most one popup per request and client. You can also run
`pde-pr-approve` directly in a real terminal.

Requests expire after four hours. A declined or expired request must be
requested again; do not approve a different command by reusing an old answer.
