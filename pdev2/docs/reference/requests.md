# States, Exit Codes, and Files

## Commands

| Command | Behavior |
|---|---|
| `pde-gh-write pr OPERATION [ARGS]` | Record the request, wait for an answer, run `gh` once |
| `pde-pr-approve REQUEST_ID` | Show a pending request in `less` and ask for `yes` or `no` |

`OPERATION` is one of `create`, `edit`, `comment`, `review`, `merge`, `ready`,
`close`, or `reopen`. Other operations and `--web` or `-w` are refused.

## States

| State | Set by | Meaning |
|---|---|---|
| `pending` | `pde-gh-write` | Waiting for an answer |
| `approved` | `pde-pr-approve` | `yes` was typed; `gh` has not started |
| `declined` | `pde-pr-approve` | `no` was typed |
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
`state`, and `exit_code`.

## Environment

| Variable | Use |
|---|---|
| `XDG_STATE_HOME` | Root of the state directory; `~/.local/state` when unset |
| `PATH` | The first `gh` is the guard, a wrapper that blocks PR writes unless `PDE_GH_WRITE=1`; the next executable `gh` that is neither the guard nor this program is the one run |
| `PDE_GH_WRITE` | Set to `1` for the approved `gh` only, so the guard lets it through |

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
| Record expiry | 4 hours after the last change |
| `gh pr view` lookup for `pr edit` | 3 seconds |
