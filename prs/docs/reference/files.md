# Cache and Temporary Files

## The Cache

The cache is `$XDG_CACHE_HOME/prs`, which is `~/.cache/prs` when
`XDG_CACHE_HOME` is not set. `prs` makes it with mode `0700`.
`internal/cache/cache.go` reads and writes it.

| File                             | Holds                                                                                        | Written by                                                        |
| -------------------------------- | -------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `list.json`                      | The PR list, with every field the details show, and `fetchedAt`, when prs got it from GitHub | The first start and each refresh that works                       |
| `OWNER+REPO#NUMBER@UPDATED.json` | One pull request's description, up to 100 files, and its comments, reviews and code threads  | The background fetch, or a screen that found it missing           |
| `OWNER+REPO#NUMBER@UPDATED.diff` | That pull request's whole diff, as `gh pr diff --color=never` prints it                      | The background fetch, or the diff screen when it found it missing |
| `NAME.tmp*`                      | A file being written, renamed to `NAME` when complete                                        | Any of the above                                                  |

## Temporary Folders

Each fzf screen's `prs` process makes a folder named `prs-*` in `$TMPDIR`,
or `/tmp`, and removes it when the screen ends, on Ctrl-C, on a hang-up and
on TERM. The hidden commands get it as `DIR`.

| Screen                          | File                    | Holds                                                                                                    |
| ------------------------------- | ----------------------- | -------------------------------------------------------------------------------------------------------- |
| PR list screen                  | `fzf.sock`              | fzf's `--listen` socket, which the refresher sends actions to                                            |
| PR list screen                  | `refresh`               | The saved list's time while a refresh runs or after one failed, which the prompt shows as the list's age |
| PR list screen                  | `error`                 | The error from the last failed refresh, from gh or from saving the list                                  |
| PR list screen, comments screen | `pane-closed`           | Exists while `o` keeps the side pane hidden                                                              |
| Every fzf screen                | `g`                     | The time of a first `g`, so a second one soon after goes to the first row                                |
| Comments screen                 | `feedback.json`         | The comments, reviews and code threads that the side pane and `_item_page` read                          |
| Diff screen                     | `1.diff`, `2.diff`, ... | Each changed file's lines from the diff, its `diff --git` header first, numbered from 1 in diff order    |
