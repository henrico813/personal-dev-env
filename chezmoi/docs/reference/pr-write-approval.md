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
waits 90 seconds for a popup answer.

Exit codes are:

- `0`: the approved `gh` command succeeded.
- `2`: the wrapper or guard rejected the command or its arguments.
- `3`: approval timed out or no terminal popup was available.
- `4`: the user declined the request.
- Any other value: passed through from the real `gh` command.

The guard blocks `gh pr create`, `new`, `edit`, `comment`, `review`, `merge`,
`ready`, `close`, and `reopen`. It also blocks write-like `gh api` calls for
pull requests, issue comments, and GraphQL mutations. Use `pde-gh-write` for
those operations. `chezmoi/dot_zshrc.tmpl` puts `~/.local/bin` before aqua so
the guard is found first.
