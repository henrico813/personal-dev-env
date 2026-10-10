# Pull-Request Approval Reference

Exact commands, files, and outcomes for full installs. Record fields,
popup keys, and the program's other modes are in the
[program reference](../../../pdev2/docs/reference/requests.md).

## Installed Files

| Path | Profile | Source |
| --- | --- | --- |
| `~/.local/bin/pde-gh-write` | full | Built from `pdev2/` by the installer |
| `~/.local/bin/pde-pr-approve` | full | Symbolic link to `pde-gh-write`, made by the installer |
| `~/.local/bin/gh` (the guard) | full | `chezmoi/dot_local/bin/executable_gh.tmpl` |
| `~/.local/share/pde/herdr-plugins/pde-approval/herdr-plugin.toml` | full | `chezmoi/dot_local/share/pde/herdr-plugins/pde-approval/herdr-plugin.toml.tmpl` |
| Herdr plugin link | full | `chezmoi/run_after_link_herdr_approval.sh.tmpl`, after every apply |

On terminal installs, chezmoi deletes `~/.local/bin/pde-gh-write` and
`~/.local/bin/pde-pr-approve` through `chezmoi/.chezmoiremove`, and the
guard because its template renders empty there.

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

A request expires four hours (14,400 seconds) after it last changed. The
wrapper waits 90 seconds for an answer from Herdr, Moshi, or the terminal.

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

## Herdr Link Messages

The link script always exits 0 and prints one of these when it cannot
link:

| Message | Cause |
| --- | --- |
| `Herdr is unavailable; link later with: herdr plugin link DIR` | `herdr` not found, or `herdr status server` failed |
| `jq is unavailable; link later with: herdr plugin link DIR` | `jq` not found |
| `Herdr plugin list failed: ERROR` | `herdr plugin list --json` failed |
| `Cannot parse the Herdr plugin list: ERROR` | `jq` could not read that list |
| `Herdr plugin unlink failed: ERROR` | Removing a link to another manifest failed |
| `Herdr plugin link failed: ERROR` | `herdr plugin link DIR` failed |

It looks for `herdr` on `PATH` and in `~/.local/bin`, where the installer
puts it.

## Phone Question

The phone ask is started as:

```text
moshi-hook ask --require-remote --source pde-gh-write --timeout 4h QUESTION
```

`--require-remote` means only the phone can answer; `moshi-hook` never
falls back to asking in a terminal. The question is
`Approve gh pr OPERATION in REPO (TARGET)[, titled TITLE]?`. REPO is the
`--repo` value when given and otherwise `current repository`. The
`, titled TITLE` part is omitted when no title was supplied.

Each request record stores the ask's process ID while it runs. If that
process is gone, a rerun starts a new ask; a running one prevents a second
ask. The ask removes the process ID when it exits.
