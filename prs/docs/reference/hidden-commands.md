# Hidden Commands

fzf runs `prs` again with a hidden command for each side pane, screen event,
tap and most keys. Each one prints fzf actions or text and exits, except the
commands that open a screen, which run until you leave it. `hiddenCommands`
in `internal/app/app.go` maps each name to its handler. `DIR` is the calling
screen's temporary folder, `REPO` is `OWNER/NAME`, and `NUMBER` is a pull
request's number. `N` is a row of the comments screen, counted from 0, for
`_item` and `_item_page`, and a file of the diff, counted from 1, for
`_file`.

## PR List Screen

| Command                 | Run by                                                                                                 | Prints or does                                                                                                | Handler                             |
| ----------------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------- | ----------------------------------- |
| `_layout DIR EVENT`     | fzf's `start`, `result` and `resize` events, `o` (`toggle`), Esc, the refresher after a failed refresh | The side pane's position and size, the help text row, and `refresh-preview` when the side pane must run again | `printListLayout` in `prlist.go`    |
| `_meta DIR REPO NUMBER` | The side pane                                                                                          | The details from the saved list, or the status strip when the side pane has 3 rows or fewer                   | `printDetails` in `prlist.go`       |
| `_tap DIR`              | A tap on the help text row                                                                             | The tapped key's action                                                                                       | `printListTapAction` in `prlist.go` |
| `_prompt DIR`           | fzf's `load` event, Esc, the refresher                                                                 | The prompt, with the list's age while it is stale                                                             | `printListPrompt` in `prlist.go`    |
| `_refresh DIR`          | `r`                                                                                                    | Signals the main `prs` process to refresh the list                                                            | `requestListRefresh` in `prlist.go` |
| `_rows`                 | The refresher's `reload` action                                                                        | The saved list's rows                                                                                         | `printListRows` in `prlist.go`      |
| `_summary REPO NUMBER`  | `l`, Enter, a double tap                                                                               | Runs the PR summary screen                                                                                    | `openSummary` in `summary.go`       |
| `_comments REPO NUMBER` | `c`                                                                                                    | Runs the comments screen                                                                                      | `openComments` in `comments.go`     |
| `_diff REPO NUMBER`     | `d`                                                                                                    | Runs the diff screen                                                                                          | `openDiff` in `diff.go`             |

## Comments Screen

| Command                                  | Run by                                                       | Prints or does                                          | Handler                                   |
| ---------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------- | ----------------------------------------- |
| `_comments_layout DIR [toggle\|refresh]` | `start`, `result`, `resize` (`refresh`), `o` (`toggle`), Esc | The side pane's position and size and the help text row | `printCommentsLayout` in `comments.go`    |
| `_item DIR N`                            | The side pane                                                | Row `N` in full                                         | `printItem` in `comments.go`              |
| `_item_page DIR N`                       | `l`, Enter, a double tap                                     | Shows row `N` full screen in less                       | `openItem` in `comments.go`               |
| `_comments_tap DIR NUMBER`               | A tap on the help text row                                   | The tapped key's action                                 | `printCommentsTapAction` in `comments.go` |

## File Picker

| Command                        | Run by                                       | Prints or does                                                 | Handler                                 |
| ------------------------------ | -------------------------------------------- | -------------------------------------------------------------- | --------------------------------------- |
| `_picker_layout DIR [refresh]` | `start`, `result`, `resize` (`refresh`), Esc | The side pane's position and size and the help text row        | `printFilePickerLayout` in `diff.go`    |
| `_file DIR N`                  | The side pane                                | File `N`'s diff, colored and wrapped for the side pane's width | `printFileDiff` in `diff.go`            |
| `_picker_tap DIR NUMBER`       | A tap on the help text row                   | The tapped key's action                                        | `printFilePickerTapAction` in `diff.go` |

## Every fzf Screen

| Command             | Run by                                            | Prints or does                                                                                                                         | Handler                           |
| ------------------- | ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------- |
| `_g DIR`            | `g`                                               | fzf's `first` action when it is the second `g` within a second                                                                         | `printFirstOnSecondG` in `fzf.go` |
| `_help SCREEN`      | `?`                                               | `preview(_keys SCREEN)` when the side pane has at least as many rows as the longest fzf screen's help, or `execute(_keys_page SCREEN)` | `printHelpAction` in `help.go`    |
| `_keys SCREEN`      | `_help`, and the installer's check of a new build | The screen's help                                                                                                                      | `printHelp` in `help.go`          |
| `_keys_page SCREEN` | `_help`                                           | Shows the screen's help in less                                                                                                        | `openHelpPage` in `help.go`       |

`SCREEN` is `list`, `comments` or `picker`. Any other value prints the PR
list screen's help.

## Commands You Run

| Command                 | What it does                                                                 |
| ----------------------- | ---------------------------------------------------------------------------- |
| `prs`                   | Opens the PR list screen                                                     |
| `prs NUMBER`            | Opens the PR summary screen of that pull request in this folder's repository |
| `prs OWNER/REPO NUMBER` | Opens the PR summary screen of that pull request from anywhere               |

A hidden command typed without its arguments gets empty ones rather than a
crash. Most then print little or nothing, but `_summary`, `_comments` and
`_diff` call gh with the empty arguments and show gh's error.
