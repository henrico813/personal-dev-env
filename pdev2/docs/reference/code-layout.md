# Where the Code Lives

The code is in four packages under `internal/`; each package's `doc.go`
says what it holds. `main.go` only calls `cmd.Main`, and the installer
builds the program from `pdev2/` with `go build .`.

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
| Where the open popup's workspace is saved | `internal/approval/popup.go` |
| How the popup looks | `internal/terminal/popup.go` |
| What a key or tap does in the popup | `internal/terminal/input.go` |
| The question of `pde-pr-approve ID` | `internal/terminal/prompt.go` |
| What happens after an answer | `internal/cmd/answer.go` |
| The Herdr popup request, notification, or sidebar marker | `internal/cmd/herdr.go` |
| How the inbox checks other Herdr machines | `internal/cmd/machines.go` |
| The popup that shows other machines | `internal/terminal/others.go` |
| How `gh` is found and run | `internal/cmd/gh.go` |
| The wait, rerun results, and exit codes | `internal/cmd/writecmd.go` and `internal/cmd/cmd.go` |
| An option that a new `gh` adds to a `gh pr` command | `targetFlags` in `internal/approval/request.go`, with whether it takes a value; title, body, body file, and repository options are in `valueFlags` |

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

### What the Tests Protect

Each approval promise, and the tests that fail when it breaks:

| Promise | Tests |
|---|---|
| gh posts the text the approver saw | `TestParseReadsEveryFlagForm`, `TestPostsApprovedTextAfterFileChanges` |
| A request whose meaning to gh is unclear is refused | `TestParseRejectsUnsafeRequests` |
| An approval covers one command, text, and directory | `TestRequestIDChangesWithPostedText`, `TestOtherDirectoryNeedsNewApproval` |
| Only a typed yes or a second tap approves | `TestPromptNeedsDeliberateAnswer`, `TestPopupRejectsLoneY`, `TestPopupDiscardsEarlyInput`, `TestPopupAppliesTapAndSettleRules` |
| A later answer cannot change a decision | `TestLateYesKeepsDeclinedRequest`, `TestMoshiAnswersPendingRequest` |
| A declined or interrupted request never runs gh | `TestDecidedRequestsNeverRunGH` |
| gh runs at most once per approval | `TestRerunReportsResultWithoutRunningAgain`, `TestConcurrentRunsCallGHOnce`, `TestRerunSurvivesFailedRepoLookup` |
| An approval expires four hours after its last change | `TestExpiredApprovalNeedsNewAnswer` |
| Restarting the runner or phone question keeps the request's age | `TestRunnerRestartKeepsRequestAge`, `TestMoshiRerunRestartsDeadAsk` |
| An approval given after the agent stops waiting still runs | `TestRunnerSavesApprovedOutputForRerun`, `TestApprovalRestartsDeadRunner` |
| Only the real gh runs, never one in the checkout | `TestRealGHIgnoresEmptyPATHEntry` |
| Ctrl+b a opens the popup or logs why it cannot | `TestInboxFindsHerdrBesideItself`, `TestMainPicksEntryMode` |
