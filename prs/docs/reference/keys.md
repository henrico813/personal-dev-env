# Keys and Help Text Rows

## Words

| Word              | Meaning                                                                         |
| ----------------- | ------------------------------------------------------------------------------- |
| PR list screen    | The first screen, the open pull requests, in fzf                                |
| PR summary screen | One pull request's details, files and description, in less                      |
| comments screen   | One row per comment, review or code thread, in fzf                              |
| diff screen       | The diff view and its file picker, run by one `prs` process                     |
| diff view         | One file's diff, full screen, in less                                           |
| file picker       | The changed files, in fzf, with the selected file's diff beside or below        |
| side pane         | The part of an fzf screen that shows more about the selected row                |
| help text row     | The bottom line of a screen, naming its main keys                               |
| details           | The selected pull request's state, title, repository, counts, checks and labels |
| status strip      | The details cut to 3 rows, for a short screen                                   |

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
- `o` opens the file picker from the diff view and closes it in the picker.
  The PR summary screen has no `o`.
- fzf binds single keys, so `gg` is two presses of `g` within a second. In
  less one `g` goes to the top.

## Keys per Screen

| Screen            | Key                    | What it does                                        |
| ----------------- | ---------------------- | --------------------------------------------------- |
| PR list screen    | `l`, Enter, double tap | Open the PR summary screen                          |
| PR list screen    | `c`                    | Open the comments screen                            |
| PR list screen    | `d`                    | Open the diff screen                                |
| PR list screen    | `r`                    | Refresh the list now                                |
| PR list screen    | `h`                    | Nothing, since there is no level above              |
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

`o` in the file picker closes the picker, so the picker's side pane always
shows.

## Help Text Rows

Each row has a long form and a short form. When the long form does not fit,
the short form drops `/ find` from an fzf row, or `J K page` or `d u page`
from a less row. The file picker's long form fits 37 columns.

| Screen            | At 50 columns                                    | At 37 columns                        |
| ----------------- | ------------------------------------------------ | ------------------------------------ |
| PR list screen    | `l open  d diff  c comments  / find  ? help`     | `l open  d diff  c comments  ? help` |
| PR summary screen | `c comments  d diff  J K page  h back  ? help`   | `c comments  d diff  h back  ? help` |
| Comments screen   | `l open  o pane  h back  / find  ? help`         | `l open  o pane  h back  ? help`     |
| A comment in less | `h back  / find`                                 | `h back  / find`                     |
| Diff view         | `o files  L next  d u page  h back  ? help  1/9` | `o files  L next  h back  ? help`    |
| File picker       | `l open  o close  / find  ? help`                | `l open  o close  / find  ? help`    |
| Help in less      | `keys  h back`                                   | `keys  h back`                       |

`o pane` is the short form of "side pane". The diff view's row ends with the
file's number, such as `1/9`, and on a wider screen its name, when they fit.
The short row adds the number and the name only together, so between about
38 and 43 columns it shows neither.

Other rows of the fzf screens:

| When                                         | Row                                                                                              |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| A search is open                             | `Esc cancel  Enter open`                                                                         |
| The PR list is empty                         | `No open pull requests  r refresh  q quit`, or `No pull requests  r refresh  q quit` when narrow |
| The saved list is empty and a refresh failed | `GitHub failed  r refresh  q quit`                                                               |

## Taps

In the fzf screens a tap on a word, or on the half of a gap next to it, runs
that word's key, since a fingertip covers several columns.
`internal/layout/layout.go` finds the word with `FindTappedKey`. less does
not report taps, so the PR summary screen and the diff view cannot be
tapped.

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
less otherwise. In a less screen, `?` shows the help in less. Each help fits
35 columns, so it fits Herdr's 37x14 popup.
