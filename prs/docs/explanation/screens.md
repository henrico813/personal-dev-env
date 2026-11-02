# How the Screens Hand Off

`prs` has four screens, and none of them is drawn by `prs` itself. fzf draws
the PR list screen, the comments screen and the file picker. less draws the
PR summary screen, the diff view, a comment shown full screen, and the help
of every less screen, and of an fzf screen whose side pane is too short for
it. `prs` decides what each one shows and which screen comes next. This page
explains how the programs take turns and why.

```text
prs                     (your command)
├─ refresher, inside this prs process
└─ fzf: PR list screen
   │ side pane: prs _meta   (reads cache)
   │ start, result, resize: prs _layout
   ├─ l ─► prs _summary
   │       └─ loop: less, PR summary screen
   │          ├─ c ─► comments screen
   │          └─ d ─► diff screen
   ├─ c ─► prs _comments
   │       └─ fzf: comments screen
   └─ d ─► prs _diff
           └─ loop, keeping the file number:
              ├─ less: diff view
              └─ fzf: file picker
```

## fzf Asks prs What to Do

fzf can run a command on a key or an event and act on what it prints. `prs`
uses that for almost everything in an fzf screen. When the phone's keyboard
comes up, fzf sends a `resize` event, runs `prs _layout DIR resize`, and
`prs` prints actions such as
`change-preview-window(down,3,border-top,nowrap,noinfo)+change-footer(...)`.
The PR list screen's side pane runs `prs _meta DIR`, which reads the
selected row from `$FZF_CURRENT_ITEM`. A tap on the help text row runs
`prs _tap SCREEN DIR`, which reads the tapped column from
`$FZF_CLICK_FOOTER_COLUMN` and prints `trigger(KEY)` for the word under it,
so fzf runs that key's own binding.

Each of these is a new, short `prs` process, so nothing is kept in memory
between them. Anything that has to last, such as whether `o` closed the side
pane or when the first `g` was pressed, is a small file in the screen's
temporary folder. [Cache and temporary files](../reference/files.md) lists
them.

Each fzf screen keeps one table of key actions, such as `buildListActions`
in `internal/app/prlist.go`, which gives fzf its `--bind` flags. Letters are
commands, as in vim, so `/` unbinds every one-letter key of the table for
typing and changes the help text row to `Esc cancel  Enter open`. Esc binds
them again.

## less Says Which Key Ended It

less has no way to run a command on a key and keep going. It can quit with a
chosen exit status, through a lesskey file. `openInLess` in
`internal/app/less.go` gives less a key file on descriptor 3, through a
pipe, so no file is written. In it, each screen key is `X quit X`, so less
exits with the letter's character code as its status. When less ends, `prs`
reads the status and opens the next screen. `h` and `q` quit, so `prs` goes
back.

That is why the less screens are loops in `prs`. The PR summary screen runs
less, and when you press `c`, `prs` runs the comments screen and then less
again. less starts at the top each time, because it does not report where it
was. The same happens after `r` and `?`.

less does not report taps either, and binding them broke swipe scrolling. So
the help text row of a less screen names its keys, and a tap on it does
nothing.

## One Process for the Diff Screen

The diff screen is two programs that look like one screen. The diff view is
less, which wraps and scrolls well and has the search keys people know. The
file picker is fzf, which lists files and shows the selected file's diff in
its side pane.

`showDiff` in `internal/app/diff.go` holds the split diff and the current
file number while it swaps the two. The diff view's keys `L`, `H`, `o`, `r`
and `?` quit less with their letter. The file picker's keys print their name
with fzf's `print` action before fzf ends, so `prs` reads the key and then
the chosen row. `l` sets the current file, and `o` or `h` keep it. Because
one process owns the number, the picker opens on the file you were reading,
and closing it brings that file back.

fzf cannot hide its own list, so a diff below the file list never gets the
whole screen. That is why reading happens in the diff view, and the picker
is where you choose.

## Leaving Nothing Behind

A dropped SSH connection sends a hang-up to every process on the terminal.
Each `prs` process that made a temporary folder catches a hang-up and TERM
in `runInTempDir` in `internal/app/terminal.go`, stops its background work,
removes the folder and exits. `Run` catches Ctrl-C and does nothing with it,
so fzf, less and delta handle Ctrl-C as usual and `prs` cleans up when they
return. Ignoring Ctrl-C would not work, because an ignored signal stays
ignored in the programs `prs` starts.
