# States, Exit Codes, and Files

## Commands

| Command | Behavior |
|---|---|
| `pde-gh-write pr OPERATION [ARGS]` | Record the request, wait for an answer, run `gh` once |
| `pde-pr-approve REQUEST_ID` | Show a pending request in `less` and ask for `yes` or `no` |
| `pde-pr-approve --herdr-popup` | Run the Herdr popup for the request in `PDE_REQUEST_ID` |
| `pde-gh-write --moshi-ask ID QUESTION` | Hidden: the detached phone ask |

`OPERATION` is one of `create`, `edit`, `comment`, `review`, `merge`, `ready`,
`close`, or `reopen`. Other operations and `--web` or `-w` are refused.

## States

| State | Set by | Meaning |
|---|---|---|
| `pending` | `pde-gh-write` | Waiting for an answer |
| `approved` | An answer | Approved in a terminal, popup, or phone; `gh` has not started |
| `declined` | An answer | Declined in a terminal, popup, or phone |
| `running` | `pde-gh-write` | `gh` started; if found later, that run died |
| `ran` | `pde-gh-write` | `gh` exited; `exit_code` holds its code |

## pde-gh-write Exit Codes

| Code | Meaning |
|---|---|
| `0` | `gh` succeeded, now or in an earlier run |
| `1` | This program failed, such as no real `gh` or an unreadable record |
| `2` | The command or its arguments were refused |
| `3` | No answer within 90 seconds; rerun after approval |
| `4` | The request was declined |
| `6` | An earlier run was interrupted; it is never retried |
| other | Passed through from `gh` |

`gh`'s own exit codes pass through too, and `gh` also uses 1, 2, and 4. Lines
on standard error that start with `pde-gh-write:` come from this program;
anything else came from `gh`.

Rerun status lines are `already ran ID, exit N`, `declined ID`, and
`interrupted ID`.

## pde-pr-approve Exit Codes

| Code | Meaning |
|---|---|
| `0` | Answer recorded; another answer arrived or the request expired while `less` was open; or input closed with no answer |
| `1` | No pending, unexpired request with that ID; no terminal; `less` failed; or the answer could not be read or saved |
| `2` | Missing or extra arguments |

## Herdr Popup

| Input | Effect |
|---|---|
| Tap `APPROVE` or `DECLINE` | Approve or decline (left button) |
| `yes` or `no`, then Enter | Approve or decline; any case; anything else asks again |
| Backspace or Delete | Remove the last typed character |
| `v` | Show the full request in `less`; leftover keys are discarded after it closes |
| `q` or Esc | Close without answering; the request stays pending |

Input typed before the screen draws is discarded. The pane is 40x16 in
`plugin/herdr-plugin.toml`, leaving 37x14 inside the border. Below 11 rows
the buttons are one row tall; below 7 rows only the header is drawn.

`pde-pr-approve --herdr-popup` exits 1 when `PDE_REQUEST_ID` is empty or the
request is not pending, 0 when closed without an answer, and otherwise as
`pde-pr-approve` does after recording an answer.

## Phone Ask

The ask runs:

```bash
moshi-hook ask --require-remote --source pde-gh-write --timeout 4h QUESTION
```

| moshi-hook exit | Result |
|---|---|
| `0` | Approve, if the request is still pending |
| `1` | Decline, if the request is still pending |
| other, or not started | No answer |

`--moshi-ask` itself exits 1 for wrong arguments or when the record cannot be
locked or saved, and 0 otherwise.

## Files

Requests are saved in the state directory,
`${XDG_STATE_HOME:-~/.local/state}/pde/pr-write/`. It is created with mode
0700 and the files with 0600.

| File | Purpose |
|---|---|
| `ID.json` | The request record |
| `ID.json.lock` | Lock file; only the program holding it may change the record or run `gh` |
| `.record-*` | Temporary file while a record is saved |

Record fields are `id`, `updated`, `args` (the arguments given to `gh`),
`title`, `body`, `body_file`, `approval_text` (what the approver sees),
`state`, `exit_code`, and `moshi_pid` (the live phone ask, omitted when none).

## Environment

| Variable | Use |
|---|---|
| `XDG_STATE_HOME` | Root of the state directory; `~/.local/state` when unset |
| `PATH` | The first `gh` is the guard, a wrapper that blocks PR writes unless `PDE_GH_WRITE=1`; the next executable `gh` that is neither the guard nor this program is the one run |
| `PDE_GH_WRITE` | Set to `1` for the approved `gh` only, so the guard lets it through |
| `HERDR_ENV` | When `1`, a new request opens the Herdr popup and a notification |
| `PDE_REQUEST_ID` | Set by Herdr for `--herdr-popup`: the request to show |

## Request ID

The ID is the SHA-256 of the JSON array `[command, title, body]`:

- `command` is `gh` followed by each original argument quoted, as shown to the
  approver.
- `title` is the value of `--title`, `-t`, or `--subject`.
- `body` is the value of `--body` or `-b`, or the contents of the
  `--body-file` or `-F` file.

## PR Number in pr edit

For `pr edit`, the approval screen shows the PR's current title and body
next to the requested ones. To fetch them, the program takes the first
argument that is not an option or an option's value as the PR number or
branch. In `gh pr edit --base main 12`, that is `12`.

The `"edit"` entry of `targetFlags` in `parse.go` lists the options that
may come before the number and whether each takes a value. Title, body,
body file, and repository options are in `valueFlags`. When `gh` adds any
option, add it to the list, marked with whether it takes a value; otherwise
commands that put it before the number show `Cannot show current values: the
PR number or branch is unclear.` instead of the old values. The write itself
still works.

## Limits

| Limit | Value |
|---|---|
| Wait for an answer | 90 seconds |
| Record expiry | 4 hours after the last change; saving `moshi_pid` alone is not a change |
| `gh pr view` lookup for `pr edit` | 3 seconds |
