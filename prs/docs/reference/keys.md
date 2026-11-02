# Keys and Help Text Rows

## Words

| Word              | Meaning                                                                                                  |
| ----------------- | -------------------------------------------------------------------------------------------------------- |
| PR list screen    | The first screen, the open pull requests, in fzf                                                         |
| PR summary screen | One pull request's details, files and description, in less                                               |
| comments screen   | One row per comment, review or code thread, in fzf                                                       |
| diff screen       | The diff view and its file picker, run by one `prs` process                                              |
| diff view         | One file's diff, full screen, in less                                                                    |
| file picker       | The changed files, in fzf, with the selected file's diff beside or below                                 |
| side pane         | The part of an fzf screen that shows more about the selected row                                         |
| help text row     | The line naming a screen's main keys, under the list in an fzf screen and at the bottom of a less screen |
| details           | The selected pull request's state, title, repository, counts, checks and labels                          |
| status strip      | The details cut to 3 rows, for a short screen                                                            |

## Keys on Most Screens

| Key        | What it does                                                             |
| ---------- | ------------------------------------------------------------------------ |
| `j` / `k`  | Move down and up a row in fzf, or scroll a row in less                   |
| `l`        | Open the selected thing                                                  |
| `h`        | Go back out one level                                                    |
| `q`        | Go back one level, or quit from the PR list screen                       |
| `o`        | Show or hide the side pane                                               |
| `gg` / `G` | First and last row, or the top and bottom of the text                    |
| `J` / `K`  | Scroll the side pane half a page in fzf, or the text half a page in less |
| `/`        | Find, and Esc cancels it                                                 |
| `?`        | Show the screen's keys                                                   |

The exceptions:

- `l` does nothing in a less screen, which has nothing to open.
- `h` does nothing on the PR list screen, which has no level above it.
- The PR summary screen has no `o`.
- fzf binds single keys, so `gg` is two presses of `g` within a second. In
  less one `g` goes to the top.

## Keys per Screen

| Screen            | Key                    | What it does                                        |
| ----------------- | ---------------------- | --------------------------------------------------- |
| PR list screen    | `l`, Enter, double tap | Open the PR summary screen                          |
| PR list screen    | `c`                    | Open the comments screen                            |
| PR list screen    | `d`                    | Open the diff screen                                |
| PR list screen    | `r`                    | Refresh the list now                                |
| PR summary screen | `c`                    | Open the comments screen                            |
| PR summary screen | `d`                    | Open the diff screen                                |
| PR summary screen | `h`, `q`               | Go back                                             |
| Comments screen   | `l`, Enter, double tap | Show the selected row full screen in less           |
| Comments screen   | `h`, `q`               | Go back                                             |
| A comment in less | `h`, `q`               | Go back to the comments screen                      |
| Diff view         | `L` / `H`              | Next and previous file                              |
| Diff view         | `o`                    | Open the file picker                                |
| Diff view         | `r`                    | Draw the diff again at the screen's width           |
| Diff view         | `d` / `u`              | Half a page down and up                             |
| Diff view         | `h`, `q`               | Leave the diff screen                               |
| File picker       | `j` / `k`              | Choose a file, and its diff follows                 |
| File picker       | `l`, Enter, double tap | Open the file in the diff view and close the picker |
| File picker       | `o`, `h`               | Close the picker on the file you were reading       |
| File picker       | `q`                    | Leave the diff screen                               |

## Help Text Rows

Most rows have a short form that drops `/ find`, `J K page` or `d u page`
when the long form does not fit.

## Taps

In the fzf screens a tap on a word, or on the half of a gap next to it, runs
that word's key, since a fingertip covers several columns.
`internal/layout/layout.go` finds the word with `FindTappedKey`.

## Layout by Screen Size

| Screen                       | Size                           | Side pane                                                                  |
| ---------------------------- | ------------------------------ | -------------------------------------------------------------------------- |
| PR list screen               | Over 100 columns               | Right, 50%                                                                 |
| PR list screen               | Under 30 rows, over 80 columns | Right, 45%                                                                 |
| PR list screen               | Under 30 rows                  | The status strip, or the details when 8 or more rows fit below the matches |
| PR list screen               | 30 rows or more                | Below; the list takes a row per match, up to half the screen               |
| Comments screen, file picker | Over 100 columns               | Right, 60%                                                                 |
| Comments screen, file picker | 100 columns or fewer           | Below a list of 5 rows, or 3 rows under 30 rows                            |

In an fzf screen, `?` shows the help in the side pane when the side pane has
at least as many rows as the longest fzf screen's help, and full screen in
less otherwise. In a less screen, `?` shows the help in less.
