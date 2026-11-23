# Where the Code Lives

The code is in six packages under `internal/`; each package's `doc.go`
says what it holds. Three programs are built from `pdev/`:

- `pde-gh`, from `cmd/pde-gh/`, only calls `ghtool.Main`. The installer
  links `gh`, `pde-gh-guard` and `pde-gh-write` to it.
- `pde-pr-approve`, from `main.go`, only calls `cmd.Main`.
- `pde-x-watch`, from `cmd/pde-x-watch/`, only calls `xwatch.WatchDisplay`.

`approval` imports no other pdev package, and `terminal` imports only
`approval`. `ghtool` imports only `approval`, and only `cmd/pde-gh`
imports `ghtool`; `TestToolFamiliesImportOnlySharedPackages` in
`imports_test.go` fails otherwise. `cmd` imports `approval` and `terminal`.
`xwatch` imports no other pdev package, and only `cmd/pde-x-watch` imports it.
`pde-gh` and `pde-pr-approve` reach each other by starting one another from their own
folder, never by import. The popup, the terminal prompt, and the phone all
return the same answers, `approval.ChoiceYes` and the others, and `cmd`
saves them.

## How the Programs Start

`ghtool.Main` picks the mode from the program name and the first argument,
and `cmd.Main` from the first argument:

| Command | File |
|---|---|
| `gh ...` and `pde-gh-guard ...` | `internal/ghtool/guard.go` |
| `pde-gh-write pr ...` | `internal/ghtool/writecmd.go` |
| `pde-gh --pde-runner ID`, the runner that runs a late approval | `internal/ghtool/runnercmd.go` |
| `pde-pr-approve ID` | `internal/cmd/approvecmd.go` |
| `pde-pr-approve --herdr-popup` and `--herdr-inbox` | `internal/cmd/popupcmd.go` |
| `pde-pr-approve --herdr-notify ID`, the popup and marker for a request | `internal/cmd/herdr.go` |
| `pde-pr-approve --moshi-ask ID`, the phone question | `internal/cmd/moshiaskcmd.go` |

`ghtool.go` and `cmd.go` hold each `Main` and its exit codes, and what each
program starts of the other. `answer.go` saves an answer from the terminal,
the popup, or the phone. `approval/process.go` starts and stops the
detached programs. `ghtool/gh.go` and `cmd/herdr.go` hold the calls to gh
and herdr.

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
| Which `gh` commands the guard refuses | `internal/ghtool/guard.go` |
| How the inbox checks other Herdr machines | `internal/cmd/machines.go` |
| The popup that shows other machines | `internal/terminal/others.go` |
| How `gh` is found and run | `internal/ghtool/gh.go` |
| The X watch's limits, how often it checks, or its notice text | `internal/xwatch/watch.go` |
| How the X watch counts each client's windows | `internal/xwatch/xres.go` |
| How a merge or a new PR keeps a stack in order, and the merge of a PR in a GitHub stack | `internal/ghtool/stack.go` |
| The wait, rerun results, and exit codes | `internal/ghtool/writecmd.go` and `internal/ghtool/ghtool.go` |
| An option that a new `gh` adds to a `gh pr` command | `targetFlags` in `internal/approval/request.go`, with whether it takes a value; title, body, body file, and repository options are in `valueFlags` |

## Changes That Lose Waiting Requests

A request that is waiting when pdev is rebuilt must still be found by the
new build. The request ID is a hash of the command, title, body, and
directory (`approval.RequestID`), and the record is saved as JSON with the
tags in `approval.Record`. Changing either makes the new build miss
requests saved by the old one.

## Tests

Tests sit in the package of the code they check. Tests in `cmd` and
`ghtool` replace package variables, such as `answerTimeout`, and set
environment variables such as `PATH` and `XDG_STATE_HOME`, so they do not
run in parallel.

### What the Tests Protect

Each approval promise, and the tests that fail when it breaks:

| Promise | Tests |
|---|---|
| gh posts the text the approver saw | `TestParseReadsEveryFlagForm`, `TestPostsApprovedTextAfterFileChanges` |
| A request whose meaning to gh is unclear is refused | `TestParseRejectsUnsafeRequests` |
| An approval covers one command, text, and directory | `TestRequestIDChangesWithPostedText`, `TestOtherDirectoryNeedsNewApproval` |
| Only a typed yes or a second tap approves | `TestPromptNeedsDeliberateAnswer`, `TestPopupRejectsLoneY`, `TestPopupDiscardsEarlyInput`, `TestPopupAppliesTapAndSettleRules` |
| A later answer cannot change a decision | `TestLateYesKeepsDeclinedRequest`, `TestMoshiAnswersPendingRequest` |
| A request answered in the popup within 30 seconds never reaches the phone | `TestPhoneWaitsForPopupInHerdr` |
| A declined or interrupted request never runs gh | `TestDecidedRequestsNeverRunGH` |
| A dismissed request comes back when the agent asks again, and Esc or Up never closes the popup | `TestRerunShowsDismissedRequestAgain`, `TestPopupIgnoresEscKey` |
| gh runs at most once per approval | `TestRerunReportsResultWithoutRunningAgain`, `TestConcurrentRunsCallGHOnce`, `TestRerunSurvivesFailedRepoLookup` |
| An approval expires four hours after its last change | `TestExpiredApprovalNeedsNewAnswer` |
| Only a failed run can be asked again, and the rerun says how | `TestRetryAsksAgainOnlyAfterFailure`, `TestFailedRunRerunNamesRetryOption` |
| Restarting the runner or phone question keeps the request's age | `TestRunnerRestartKeepsRequestAge`, `TestMoshiRerunRestartsDeadAsk` |
| An approval given after the agent stops waiting still runs | `TestRunnerSavesApprovedOutputForRerun`, `TestApprovalRestartsDeadRunner` |
| Only the real gh runs, never one in the checkout | `TestRealGHIgnoresEmptyPATHEntry` |
| `gh` refuses pull request writes, and its names pick the right mode | `TestFindRefusalCoversWritesAndReads`, `TestMainPicksModeByName` |
| Ctrl+b a opens the popup or logs why it cannot | `TestInboxFindsHerdrBesideItself`, `TestMainPicksEntryMode` |
| One approval merges one PR of a stack, never the PRs below it | `TestMergeRefusesMidStackPR` |
| A PR in a GitHub stack merges through GitHub's merge API for stacks | `TestStackedMergeUsesMergeAsyncAPI` |
| After a merge, PRs based on its branch move to its base, unless GitHub moves them | `TestMergeMovesUnstackedDependentsToBase`, `TestMergeStackedFailureReportedWithoutMoves` |
| A plain merge says the PR merged, unless `--auto` only queued it | `TestMergePlainReportsMergeUnlessAuto` |
| A failed merge call counts as a merge only when GitHub then shows the PR merged | `TestMergeFailureFollowsPRState` |
| A new PR's base branch is the default branch or has an open PR | `TestCreateRefusesBaseWithoutOpenPR` |
