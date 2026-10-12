# Where the Code Lives

This page is for whoever reviews or changes pdev2. The code is split into
four packages under `internal/`, so the file paths in a change say which
part of the program it touches. `main.go` only calls `cmd.Main`, and the
installer still builds the program from `pdev2/` with `go build .`.

## Packages

| Package | What it holds | Outside programs it runs |
|---|---|---|
| `internal/approval` | Reading `gh pr` arguments into a request, the request ID, the saved record and its lock, the queue order, and the text the approver reads | `diff`, for the changes of a `pr edit` |
| `internal/terminal` | The Herdr popup's screen, keys, and taps; the yes-or-no prompt of `pde-pr-approve ID`; raw terminal mode and the pager | `less` |
| `internal/cmd` | Each way the program starts, saving answers, and every call to `gh`, `herdr`, and `moshi-hook` | `gh`, `herdr`, `moshi-hook`, and copies of itself |
| `internal/approvaltest` | Helpers for tests only: the pseudo-terminal the popup tests type into, and fake `gh`, `herdr`, and `moshi-hook` scripts | None; it is not built into the program |

`approval` imports no other pdev2 package, and `terminal` imports only
`approval`. `cmd` imports both. The popup, the terminal prompt, and the
phone all return the same answers, `approval.ChoiceYes` and the others, and
`cmd` saves them.

## How the Program Starts

`cmd.Main` picks the mode from the program name and the first argument:

| Command | File in `internal/cmd/` |
|---|---|
| `pde-gh-write pr ...` | `writecmd.go` |
| `pde-pr-approve ID` | `approvecmd.go` |
| `pde-pr-approve --herdr-popup` and `--herdr-inbox` | `popupcmd.go` |
| `pde-gh-write --pde-runner ID`, the runner that runs a late approval | `runnercmd.go` |
| `pde-gh-write --moshi-ask ID`, the phone question | `moshiaskcmd.go` |

`cmd.go` holds `Main` and the exit codes. `answer.go` saves an answer from
the terminal, the popup, or the phone. `gh.go` and `herdr.go` hold the
calls to those programs.

## Where a Change Usually Goes

| To change | Look in |
|---|---|
| Which `gh pr` commands and options are accepted, or how the PR number is found | `internal/approval/request.go` |
| What a record saves, when it expires, or the queue order | `internal/approval/record.go` |
| The context lines on the popup and the phone, or the approval text | `internal/approval/text.go` |
| How the popup looks | `internal/terminal/popup.go` |
| What a key or tap does in the popup | `internal/terminal/input.go` |
| The question of `pde-pr-approve ID` | `internal/terminal/prompt.go` |
| What happens after an answer | `internal/cmd/answer.go` |
| The Herdr popup request, notification, or sidebar marker | `internal/cmd/herdr.go` |
| How `gh` is found and run | `internal/cmd/gh.go` |
| The wait, rerun results, and exit codes | `internal/cmd/writecmd.go` and `internal/cmd/cmd.go` |

## Changes That Lose Waiting Requests

A request that is waiting when pdev2 is rebuilt must still be found by the
new build. The request ID is a hash of the command, title, body, and
directory (`approval.RequestID`), and the record is saved as JSON with the
tags in `approval.Record`. Changing either makes the new build miss
requests saved by the old one.

## Tests

Tests sit in the package of the code they check. Tests in `cmd` replace
package variables, such as `answerTimeout`, and set environment variables
such as `PATH` and `XDG_STATE_HOME`, so they do not run in parallel.
