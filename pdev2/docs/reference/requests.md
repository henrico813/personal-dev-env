# States, Exit Codes, and Files

## Commands

| Command | Behavior |
|---|---|
| `pde-gh-write pr OPERATION [ARGS]` | Record the request, wait for an answer, run `gh` once |
| `pde-pr-approve REQUEST_ID` | Show a pending request in `less` and ask for `yes` or `no` |
| `pde-pr-approve --herdr-popup` | Run by Herdr to draw the popup, starting with the request in `PDE_REQUEST_ID` |
| `pde-pr-approve --herdr-inbox` | Open the popup on the oldest pending request, dismissed or not |
| `pde-gh-write --moshi-ask ID` | Not for people: started by `pde-gh-write` to send the phone question and save the answer |
| `pde-gh-write --pde-runner ID` | Not for people: started by `pde-gh-write` to run `gh` once the request is approved |

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
on standard error that start with `pde-gh-write:` come from this program.
What `gh` prints, on standard output and standard error alike, is saved and
then printed on standard output after `gh` exits, so a rerun can print it
again.

Status lines on standard error:

| Line starts with | When |
|---|---|
| `pde-gh-write: waiting for approval` | A new request was saved; the line holds its ID |
| `pde-gh-write: still waiting for approval of ID` | No answer within 90 seconds; exit 3 |
| `pde-gh-write: already ran ID, exit N` | A rerun of a request that ran; the saved `gh` output follows |
| `pde-gh-write: declined ID` | A rerun of a declined request; exit 4 |
| `pde-gh-write: interrupted ID` | A rerun of an interrupted request; exit 6 |

## pde-pr-approve Exit Codes

| Code | Meaning |
|---|---|
| `0` | Answer recorded; another answer arrived or the request expired while `less` was open; or input closed with no answer |
| `1` | No pending, unexpired request with that ID; no terminal; `less` failed; or the answer could not be read or saved |
| `2` | Missing or extra arguments |

## The Guard

The guard is `~/.local/bin/gh`, installed by chezmoi from
`chezmoi/dot_local/bin/executable_gh.tmpl` in the full profile only.
With `PDE_GH_WRITE=1` it runs the real `gh` with any arguments.
Otherwise it refuses:

- `gh pr create`, `new`, `edit`, `comment`, `review`, `merge`, `ready`,
  `close`, and `reopen`.
- `gh api` with a path that contains `pulls`, or `issues/` followed by
  `comments`, when the method is `POST`, `PATCH`, `PUT`, or `DELETE`, or
  when `-f`, `-F`, `--field`, `--raw-field`, or `--input` is given.
- `gh api graphql` when any argument contains `mutation`.

`pde-gh-write` accepts each refused `gh pr` command except `new`; use
`create` instead. It accepts no `gh api` command.

The real `gh` is the first executable `gh` on `PATH` that is not the
guard. `chezmoi/dot_zshrc.tmpl` puts `~/.local/bin` before aqua's folder
so the guard is found first.

| Code | Meaning |
|---|---|
| `1` | No real `gh` on `PATH`; prints `gh guard cannot find the real gh executable` |
| `2` | Refused; prints `Use pde-gh-write for pull-request writes; it asks the user for approval.` |
| other | Passed through from the real `gh` |

## Herdr Popup

| Input | Effect |
|---|---|
| First tap on `APPROVE` or `DECLINE` | Select it; shows `tap APPROVE again to confirm` or `tap DECLINE again to confirm` |
| Second tap on the selected button | Approve or decline |
| Tap elsewhere, or type | Clear the selection |
| `yes` or `no`, then Enter | Approve or decline; any case; anything else asks again |
| Backspace or Delete | Remove the last typed character |
| `v` | Show the full request in `less`; leftover keys are discarded after it closes |
| `g` | Close the popup and focus the requesting agent's pane |
| `q` or Esc | Dismiss: the request stays pending but no longer opens on its own |

Only the press of a left-button tap counts. Input typed before the screen
draws is discarded, and taps and Enter are ignored for 600 ms after each
request appears, while the popup shows `one moment...`. The popup
redraws when its size changes. The first line shows the queue position, such
as `1 of 2`; after an answer or dismissal the popup moves to the oldest
request still waiting.

The popup is 40x16 in
`chezmoi/dot_local/share/pde/herdr-plugins/pde-approval/herdr-plugin.toml.tmpl`,
leaving 37x14 inside the border. Buttons are three rows tall from 16 rows, two rows from 11
to 15, and one row below 11; below 7 rows only the first line is drawn.
Typed keys and notices share the line after the request text; when both
do not fit, only the notice is shown.

`pde-pr-approve --herdr-popup` exits 1 when `PDE_REQUEST_ID` is empty or that
request is not pending, and 1 if saving an answer fails. It exits 0 when the
queue is empty, when closed without an answer, or after `g`.

`pde-pr-approve --herdr-inbox` exits 0 after opening the popup or showing a
`No approvals waiting` notification, and 1 when the popup cannot be opened.

### Context Lines

The popup starts with these lines, and the phone question asks the first
one as a question. A line whose value is unknown is left out.

| Line | Holds | Source |
|---|---|---|
| 1 | What is asked and of which PR: `comment #12`, `merge #185`, or `create PR`; `merge PR` when the number is unclear | The command |
| 2 | The repository, or `dir: ` and the folder name when `gh` finds none | `--repo`, otherwise `gh repo view` |
| 3 | The Herdr workspace and agent title, such as `app / review login fix`; the OpenCode session ID when Herdr does not know the agent | `herdr agent list` and `herdr workspace list`, found by `OPENCODE_SESSION_ID` |
| 4 | The new title when the request sets one, otherwise the PR's current title | The command, otherwise `gh pr view` |

The lookups run once, when the request is made. Without
`OPENCODE_SESSION_ID`, line 3 is left out and Herdr is not asked. For
`pr edit`, the approval text also shows the PR's current title and body
next to the requested ones.

### Opening the Popup

When `HERDR_ENV` is `1`, `pde-gh-write` does these steps for a new request
in the background and ignores their errors:

1. It sets the `Approval waiting` marker on the agent's pane.
2. It runs `herdr status server` to find Herdr's socket and sends one
   request there: open the `approval` pane of the `pde.approval` plugin as
   a popup with the keyboard on it, with the request ID in
   `PDE_REQUEST_ID`. The plugin file tells Herdr the popup's size and to
   run `pde-pr-approve --herdr-popup` in it.
3. It shows a Herdr notification, `Approval is waiting`, with a sound.

If the request is answered before step 2, the marker is removed and no
popup opens.

## Queue, Inbox, and Marker

- The queue is every waiting request that is not dismissed, oldest first
  by creation time. One popup works through it; whichever popup you
  answer in moves on through the same queue.
- `q` or Esc dismisses a request: it keeps waiting and can still be
  answered on the phone or in a terminal, but the queue skips it.
- The inbox, `pde-pr-approve --herdr-inbox`, runs from `prefix+a`
  (Ctrl+b, then a) in Herdr. It opens the popup on the oldest waiting
  request, dismissed or not, then continues through the queue.
- While a request waits, Herdr's sidebar shows `Approval waiting` on the
  pane of the agent that asked. The pane is found by matching
  `OPENCODE_SESSION_ID` in `herdr agent list`. Without it there is no
  marker, and `g` only closes the popup.
- The marker belongs to the pane, so it stays until no waiting,
  undismissed request from that pane is left. It is set with a four-hour
  time limit.

## Phone Question

`pde-gh-write --moshi-ask` runs:

```bash
moshi-hook ask --require-remote --source pde-gh-write --timeout 4h QUESTION
```

`QUESTION` is `Approve` and the first context line, followed by the other
context lines, built from the saved record. Only these lines go to Moshi;
the body and the rest of the command stay on this computer.
`--require-remote` means only the phone can answer: `moshi-hook` never
asks in the terminal instead.

| moshi-hook exit | Result |
|---|---|
| `0` | Approve, if the request is still pending |
| `1` | Decline, if the request is still pending |
| other, or not started | No answer |

`--moshi-ask` itself exits 1 for wrong arguments or when the record cannot be
locked or saved, and 0 otherwise. An answer from the terminal or popup, or
the runner when the request expires, ends a running phone question with
`SIGTERM` to its process group.

## Runner

The runner checks the record once a second until the request is answered or
expires. The runner's exit code is not read by anything.

When the request expires, the runner ends the phone question and clears
the sidebar marker before it exits.

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
| `gh` (optional) | The real `gh` found at creation; the runner uses it |
| `created` | Creation time; orders the queue |
| `updated` | Last save; expiry counts from it |
| `state` | See [states](#states) |
| `exit_code` | `gh`'s exit code once it ran |
| `output` (optional) | Up to 64 KiB of `gh`'s standard output and error |
| `moshi_pid`, `runner_pid` (optional) | Process IDs of the running phone question and runner |
| `dismissed` (optional) | Dismissed from the popup |
| `repo` (optional) | `--repo` value, or the repository `gh` found |
| `pr_title` (optional) | Current PR title, when the request sets none |
| `session_id` (optional) | OpenCode session that asked |
| `session_title`, `workspace` (optional) | Herdr agent title and workspace label |
| `pane_id` (optional) | Herdr pane of the agent that asked |

## Environment

| Variable | Use |
|---|---|
| `XDG_STATE_HOME` | Root of the state directory; `~/.local/state` when unset |
| `PATH` | The first `gh` is [the guard](#the-guard), which blocks PR writes unless `PDE_GH_WRITE=1`; the next executable `gh` that is neither the guard nor this program is the one run |
| `PDE_GH_WRITE` | Set to `1` for the approved `gh` only, so the guard lets it through |
| `HERDR_ENV` | Herdr sets it to `1` in its terminals; then a new request opens the Herdr popup, shows a notification, sets the sidebar marker, and looks up the Herdr agent |
| `PDE_REQUEST_ID` | Set by Herdr for `--herdr-popup`: the request to show first |
| `OPENCODE_SESSION_ID` | The OpenCode session running the agent, when set by `chezmoi/dot_config/opencode/plugins/pde-approval-session.js`; recorded as `session_id` and used to find the agent's pane |

## Request ID

The ID is the SHA-256 of the JSON array
`[command, title, body, directory]`:

- `command` is `gh` followed by each original argument quoted, as shown to the
  approver.
- `title` is the value of `--title`, `-t`, or `--subject`.
- `body` is the value of `--body` or `-b`, or the contents of the
  `--body-file` or `-F` file.
- `directory` is the working directory of `pde-gh-write`.

Looked-up values, such as the repository name and the PR title, are never
part of the ID.

## PR Number

The PR number or branch is the first argument that is not an option or an
option's value. In `gh pr edit --base main 12`, that is `12`. `pr create`
has no PR number. When an option the program does not know comes before
the number, the write still works, but:

- The popup and the phone show `merge PR` instead of `merge #12`, and no
  current PR title.
- For `pr edit`, the approval screen shows `Cannot show current values: the
  PR number or branch is unclear.` instead of the old values.

## Limits

| Limit | Value |
|---|---|
| Wait for an answer | 90 seconds |
| Record expiry | 4 hours after the last change; saving `moshi_pid`, `runner_pid`, or `dismissed` alone is not a change |
| Each `gh` and Herdr lookup while creating a request | 3 seconds |
| Runner check interval | 1 second |
| Saved `gh` output | 64 KiB |
| Settle pause after each popup request appears | 600 milliseconds |
