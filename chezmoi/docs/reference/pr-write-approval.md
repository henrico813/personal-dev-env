# Pull-request approval reference

## Commands

| Command | Purpose |
| --- | --- |
| `pde-gh-write pr OPERATION [ARGS]` | Record and approve an allowlisted write |
| `pde-pr-approve [REQUEST_ID]` | Approve from a real terminal |
| `pde-pr-approve --popup-pending CLIENT [REQUEST_ID]` | Restore a popup |

State is stored under `${XDG_STATE_HOME:-~/.local/state}/pde/`:

- `pr-requests/REQUEST_ID` stores the exact command, title, and body.
- `pr-approvals/REQUEST_ID` stores one pending approval.
- `pr-approvals/.popups/` prevents duplicate popups for one client.

Requests and approvals expire after 14,400 seconds (four hours). The wrapper
waits 90 seconds for a popup answer. Exit 3 means timeout or no terminal
popup; exit 4 means decline; exit 2 means the command is not allowed.

The guard blocks `gh pr create`, `new`, `edit`, `comment`, `review`, `merge`,
`ready`, `close`, and `reopen`. It also blocks write-like `gh api` calls for
pull requests, issue comments, and GraphQL mutations. Use `pde-gh-write` for
those operations. The shell must put `~/.local/bin` before aqua so the guard
is found first.
