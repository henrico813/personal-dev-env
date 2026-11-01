# prs

`prs` lists the open pull requests that involve you in fzf, sized for a
phone screen. It reads from GitHub through `gh` and never writes. How to use
it is in the `prs` part of
[GitHub Pull Request Reviews](../chezmoi/README.md#github-pull-request-reviews).

```bash
prs                 # the PR list screen
prs 123             # one PR's summary screen, from this folder's repo
prs OWNER/REPO 123  # the same from anywhere
```

`pde-installer` builds it on full installs. It runs `gh`, `fzf` 0.74.5 or
newer and `less` 590 or newer. `delta` colors the diffs and `bat` the
descriptions; without them, `prs` shows the plain diff and an uncolored
description.

## Screens

Each screen ends in a help text row naming its main keys. In the fzf screens
a tap on a word runs it. less does not report taps, so in the less screens
the row names the keys and a tap does nothing.

- PR list screen (fzf): your open pull requests, with the selected one's
  details in the side pane, or the 3-row status strip on a short screen. `l`
  opens the PR summary screen, `c` the comments screen, `d` the diff screen,
  and `r` refreshes the list.
- PR summary screen (less): the details, the comment count, the files and
  the description. `c` and `d` open the comments and diff screens.
- Comments screen (fzf): one row per conversation comment, review and code
  thread, open threads first, with the selected one in the side pane. `l`
  shows it full screen.
- Diff screen: the diff view shows one file's diff full screen in less, with
  `L` and `H` for the next and previous file and `r` to redraw at a new
  width. `o` swaps in the file picker in fzf, with the selected file's diff
  in the side pane; `l` opens a file and `o` or `h` goes back.

On most screens, `j` and `k` move, `h` goes back, `q` goes back or quits
from the PR list screen, `o` opens or closes the side pane, `gg` and `G` go
to the first and last row, `J` and `K` scroll half a page, `/` finds and Esc
cancels it, and `?` shows the keys. The keys page in `docs/` lists the
exceptions.

## How it works

On screen events and most keys in an fzf screen, fzf runs `prs` again with a
hidden command, such as `prs _layout DIR resize`, and `prs` prints fzf
actions or text. In the less screens, a lesskey file, less's own list of
keys and their actions, makes each screen key quit less with its letter's
character code as the exit status, so `prs` knows which screen comes next.
The diff screen is one `prs` process that keeps the split diff and the
current file while it swaps less and fzf.

`prs` keeps the PR list in `~/.cache/prs`, with each listed pull request's
description, files, comments and diff in files named by the pull request's
last-update time. One GraphQL query fetches the list with every field the
details show. The PR list screen shows the saved list at once, then
refreshes it in place through fzf's `--listen` socket, a socket that lets
another program send fzf actions while it runs. The refresher also fetches
in the background whatever the cache lacks. While a refresh runs or after
one failed, the prompt shows the list's age, such as `updated 5s ago`.

Each package does one job:

- `internal/app`: the screens, their keys and help, the hidden commands and
  the background refresher. Start at `Run` in `app.go`. Each screen has its
  own file: `prlist.go`, `summary.go`, `comments.go` and `diff.go`, which
  holds the diff view and the file picker. `refresh.go` is the refresher,
  `fzf.go`, `less.go` and `terminal.go` run fzf, less and the terminal, and
  `help.go` shows the help.
- `internal/github`: types for gh's JSON and the gh calls.
- `internal/cache`: the saved list, and each pull request's description,
  files, comments and diff.
- `internal/view`: the text of each screen, in a file named like the
  screen's file in `app`, with the colors and wrapping they share in
  `view.go`.
- `internal/layout`: side panes and help text rows for a screen size.
- `internal/diffsplit`: splits a diff into one file per changed file, in the
  temporary folder the diff screen reads.
- `internal/codewrap`: rewraps diffs and other text so wrapped rows start
  under their code, breaking where nvim breaks.

To change one screen, start with its file in `internal/app`, then the file
of the same name in `internal/view`, and `internal/layout/layout.go` for its
side pane and help text row.

The pages in [docs](docs/README.md) list every key, hidden command and file,
and explain how the screens hand off and how the cache works.

## Checks

```bash
make init   # once, installs golangci-lint
make check
```
