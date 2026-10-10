# Pull-request approval reference

## Commands

| Command | Purpose |
| --- | --- |
| `pde-gh-write pr OPERATION [ARGS]` | Record and approve an allowlisted write |
| `pde-pr-approve REQUEST_ID` | Approve from a real terminal |

State is stored under `${XDG_STATE_HOME:-~/.local/state}/pde/pr-write/`:

- `REQUEST_ID.json` stores the exact command, title, body, and approval state.

For `pr edit`, `pde-gh-write` first fetches the pull request's current title
and body with a read-only `gh pr view` (adding `--repo VALUE` when the command
supplied `--repo` or `-R`) and starts the request with that diff, showing the
lookup error in that section when the fetch fails. The request ID still
depends only on the original command and requested title and body, so the
current values never change it.
Requests and approvals expire after 14,400 seconds (four hours). The wrapper
waits 90 seconds for an answer from Herdr, Moshi, or the terminal.

Exit codes are:

- `0`: the approved `gh` command succeeded.
- `1`: the wrapper failed, such as when it cannot find the real `gh`.
- `2`: the wrapper or guard rejected the command or its arguments.
- `3`: approval is still waiting; tell the user and rerun the identical command.
- `4`: the user declined the request.
- `6`: the previous run was interrupted and is final.
- Any other value: passed through from the real `gh` command.

`gh` can itself exit 1, 2, or 4. Lines on standard error that start with
`pde-gh-write:` come from the wrapper, which tells the two apart.

On reruns, the wrapper can print these status lines:

- `already ran <id>, exit N`: the request already ran; the wrapper exits N.
- `declined <id>`: the request was declined; the wrapper exits 4.
- `interrupted <id>`: the request was interrupted; the wrapper exits 6.

The guard blocks `gh pr create`, `new`, `edit`, `comment`, `review`, `merge`,
`ready`, `close`, and `reopen`. It also blocks write-like `gh api` calls for
pull requests, issue comments, and GraphQL mutations. Use `pde-gh-write` for
those operations. `chezmoi/dot_zshrc.tmpl` puts `~/.local/bin` before aqua so
the guard is found first.
