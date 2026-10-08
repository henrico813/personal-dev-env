# Approve a pull-request write

The agent prints the request ID when it asks for approval. Requests from other
agent sessions can appear too, so decline one you do not recognize.

## From the popup

Run the requested command through `pde-gh-write`. Read the exact request in
the popup. Press `y` to approve or any other key to decline. The wrapper waits
up to 90 seconds and then runs `gh` only after approval.

## From a terminal

When no tmux popup is available, run this in a real terminal:

```bash
pde-pr-approve REQUEST_ID
```

Press `y` after reviewing the request. Any other key declines it. With no ID,
`pde-pr-approve` lists all pending requests. After terminal approval, tell the
agent to rerun the identical command; if it is still waiting, it reruns on its
own. The agent sees exit 3 while waiting or timing out, exit 4 when you
decline, and the underlying `gh` result after approval.

## Decline an unknown request

Press any key other than `y` for a request you do not recognize. Requests from
other agent sessions are visible because they share the same local state.

## After reconnecting

Attach to tmux normally. The `client-attached` hook looks for pending requests
and restores at most one popup per request and client. You can also run
`pde-pr-approve` directly in a real terminal.

Requests expire after four hours. A declined or expired request must be
requested again; do not approve a different command by reusing an old answer.
