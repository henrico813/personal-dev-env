# States, Exit Codes, and Files

## Commands

| Command | Behavior |
|---|---|
| `pde-gh-write pr OPERATION [ARGS]` | Record the request, wait for an answer, run `gh` once |
| `pde-pr-approve REQUEST_ID` | Show a pending request in `less` and ask for `yes` or `no` |
| `pde-pr-approve --herdr-popup` | Run the Herdr popup, starting with the request in `PDE_REQUEST_ID` |
| `pde-pr-approve --herdr-inbox` | Open the popup on the oldest pending request, dismissed or not |
| `pde-gh-write --moshi-ask ID` | Hidden: the detached phone ask |
| `pde-gh-write --pde-runner ID` | Hidden: the detached runner |

`OPERATION` is one of `create`, `edit`, `comment`, `review`, `merge`, `ready`,
`close`, or `reopen`. Other operations and `--web` or `-w` are refused.

## States

| State | Set by | Meaning |
|---|---|---|
| `pending` | `pde-gh-write` | Waiting for an answer |
| `approved` | An answer | Approved in a terminal, popup, or phone; `gh` has not started |
| `declined` | An answer | Declined in a terminal, popup, or phone |
| `running` | Writer or runner | `gh` started; if found later, that run died |
| `ran` | Writer or runner | `gh` exited; `exit_code` and `output` hold the result |

## pde-gh-write Exit Codes

| Code | Meaning |
|---|---|
| `0` | `gh` succeeded, now or in an earlier run |
| `1` | This program failed, such as no real `gh` or an unreadable record |
| `2` | The command or its arguments were refused |
| `3` | No answer within 90 seconds; the runner runs `gh` once approved, so rerun only to read the result |
| `4` | The request was declined |
| `6` | An earlier run was interrupted; it is never retried |
| other | Passed through from `gh` |

`gh`'s own exit codes pass through too, and `gh` also uses 1, 2, and 4. Lines
on standard error that start with `pde-gh-write:` come from this program;
anything else came from `gh`.

Rerun status lines are `already ran ID, exit N`, followed by the saved `gh`
output, `declined ID`, and `interrupted ID`.

## pde-pr-approve Exit Codes

| Code | Meaning |
|---|---|
| `0` | Answer recorded; another answer arrived or the request expired while `less` was open; or input closed with no answer |
| `1` | No pending, unexpired request with that ID; no terminal; `less` failed; or the answer could not be read or saved |
| `2` | Missing or extra arguments |

## Herdr Popup

| Input | Effect |
|---|---|
| First tap on `APPROVE` or `DECLINE` | Select it; shows `tap APPROVE again to confirm` |
| Second tap on the selected button | Approve or decline |
| Tap elsewhere, or type | Clear the selection |
| `yes` or `no`, then Enter | Approve or decline; any case; anything else asks again |
| Backspace or Delete | Remove the last typed character |
| `v` | Show the full request in `less`; leftover keys are discarded after it closes |
| `g` | Close the popup and focus the requesting agent's pane |
| `q` or Esc | Dismiss: the request stays pending but no longer opens on its own |

Only the press of a left-button tap counts. Input typed before the screen
draws is discarded, and taps and Enter are ignored for the settle pause after
each request appears, while the popup shows `one moment...`. The first line
shows the queue position, such as `1 of 2`; after an answer or dismissal the
popup moves to the oldest request still waiting.

The pane is 40x16 in
`chezmoi/dot_local/share/pde/herdr-plugins/pde-approval/herdr-plugin.toml.tmpl`,
leaving 37x14 inside the border. Below 11 rows the buttons are one row tall;
below 7 rows only the first line is drawn.

`pde-pr-approve --herdr-popup` exits 1 when `PDE_REQUEST_ID` is empty or that
request is not pending, and 1 if saving an answer fails. It exits 0 when the
queue is empty, when closed without an answer, or after `g`.

`pde-pr-approve --herdr-inbox` exits 0 after opening the popup or showing a
`No approvals waiting` notification, and 1 when the popup cannot be opened.

## Phone Ask

The ask runs:

```bash
moshi-hook ask --require-remote --source pde-gh-write --timeout 4h QUESTION
```

`QUESTION` is `Approve` and the first context line, followed by the other
context lines, built from the saved record.

| moshi-hook exit | Result |
|---|---|
| `0` | Approve, if the request is still pending |
| `1` | Decline, if the request is still pending |
| other, or not started | No answer |

`--moshi-ask` itself exits 1 for wrong arguments or when the record cannot be
locked or saved, and 0 otherwise. An answer from the terminal or popup stops
a running ask with `SIGTERM` to its process group.

## Runner

The runner checks the record once a second until the request is answered or
expires.

| Runner exit | When |
|---|---|
| `0` | The request expired, or `gh` ran and succeeded |
| `1` | The record could not be read, or the saved `gh` path is missing or not executable |
| `4` | The request was declined |
| `6` | The request was found `running`: an earlier run died |
| other | Passed through from `gh` |

When `gh` already ran, the runner reports the saved exit code. After an
answer or expiry, the runner clears the sidebar marker and stops the phone
ask before it exits.

## Files

Requests are saved in the state directory,
`${XDG_STATE_HOME:-~/.local/state}/pde/pr-write/`. It is created with mode
0700 and the files with 0600.

| File | Purpose |
|---|---|
| `ID.json` | The request record |
| `ID.json.lock` | Lock file; only the program holding it may change the record or run `gh` |
| `.record-*` | Temporary file while a record is saved |

Record fields, with those omitted when empty marked as optional:

| Field | Meaning |
|---|---|
| `id` | Request ID |
| `args` | Arguments given to `gh`, with a body file changed to `--body-file -` |
| `title`, `body` | Requested title and body; `body` holds body-file contents |
| `body_file` | `body` is sent to `gh` on standard input |
| `approval_text` | What `less` shows the approver |
| `directory` (optional) | Where the request was made; `gh` runs there |
| `created` | Creation time; orders the queue |
| `gh` (optional) | The real `gh` found at creation; the runner uses it |
| `updated` | Last save; expiry counts from it |
| `state` | See [states](#states) |
| `exit_code` | `gh`'s exit code once it ran |
| `output` (optional) | Up to 64 KiB of `gh`'s standard output and error |
| `moshi_pid`, `runner_pid` (optional) | Live phone ask and runner |
| `dismissed` (optional) | Dismissed from the popup |
| `repo` (optional) | `--repo` value, or the repository `gh` found |
| `pr_title` (optional) | Current PR title, when the request sets none |
| `session_id` (optional) | OpenCode session that asked |
| `session_title`, `workspace`, `pane_id` (optional) | Herdr agent title, workspace label, and pane |

## Environment

| Variable | Use |
|---|---|
| `XDG_STATE_HOME` | Root of the state directory; `~/.local/state` when unset |
| `PATH` | The first `gh` is the guard, a wrapper that blocks PR writes unless `PDE_GH_WRITE=1`; the next executable `gh` that is neither the guard nor this program is the one run |
| `PDE_GH_WRITE` | Set to `1` for the approved `gh` only, so the guard lets it through |
| `HERDR_ENV` | When `1`, a new request opens the Herdr popup, shows a notification, sets the sidebar marker, and looks up the Herdr agent |
| `PDE_REQUEST_ID` | Set by Herdr for `--herdr-popup`: the request to show first |
| `OPENCODE_SESSION_ID` | Set by the OpenCode plugin for each command; recorded as `session_id` |

## Request ID

The ID is the SHA-256 of the JSON array
`[command, title, body, repo, directory]`:

- `command` is `gh` followed by each original argument quoted, as shown to the
  approver.
- `title` is the value of `--title`, `-t`, or `--subject`.
- `body` is the value of `--body` or `-b`, or the contents of the
  `--body-file` or `-F` file.
- `repo` is the `--repo` or `-R` value as given, or empty.
- `directory` is the working directory of `pde-gh-write`.

Looked-up values, such as the repository name and the PR title, are never
part of the ID.

## PR Number

The popup and the phone show the PR a request is for, such as `merge #12`,
and its title. For `pr edit`, the approval screen also shows the PR's
current title and body next to the requested ones. To find the PR, the
program takes the first argument that is not an option or an option's value
as the PR number or branch. In `gh pr edit --base main 12`, that is `12`.
`pr create` has no PR number.

Each operation's entry in `targetFlags` in `parse.go` lists the options from
`gh pr OPERATION --help` that may come before the number and whether each
takes a value. Short options are listed per operation because they can
differ: `-m` takes a milestone for `edit` but nothing for `merge`. Title,
body, body file, and repository options are in `valueFlags`.

When `gh` adds any option, add it to the list, marked with whether it takes
a value. Otherwise commands that put it before the number lose the PR
details; the write itself still works:

- The popup and the phone show `merge PR` instead of `merge #12`, and no
  current PR title.
- For `pr edit`, the approval screen shows `Cannot show current values: the
  PR number or branch is unclear.` instead of the old values.

## Limits

| Limit | Value |
|---|---|
| Wait for an answer | 90 seconds |
| Record expiry | 4 hours after the last change; saving `moshi_pid` or `runner_pid` alone is not a change |
| Each `gh` and Herdr lookup while creating a request | 3 seconds |
| Settle pause after each popup request appears | 600 milliseconds |
| Runner check interval | 1 second |
| Saved `gh` output | 64 KiB |
