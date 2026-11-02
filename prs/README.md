# prs

`prs` lists the open pull requests that involve you in fzf, sized for a
phone screen. It reads from GitHub through `gh` and never writes. How to use
it is in the `prs` part of
[GitHub Pull Request Reviews](../chezmoi/README.md#github-pull-request-reviews).

`pde-installer` builds it on full installs. It runs `gh`, `fzf` 0.74.5 or
newer and `less` 590 or newer. `delta` colors the diffs and `bat` the
descriptions; without them, `prs` shows the plain diff and an uncolored
description.

Each package does one job:

- `internal/app`: the screens, the hidden commands and the background
  refresher. Start at `Run` in `app.go`.
- `internal/github`: types for gh's JSON and the gh calls.
- `internal/cache`: the saved list, and each pull request's description,
  files, comments and diff.
- `internal/view`: the text of each screen.
- `internal/layout`: side panes and help text rows for a screen size.
- `internal/diffsplit`: splits a diff into one file per changed file, in the
  temporary folder the diff screen reads.
- `internal/codewrap`: rewraps diffs and other text so wrapped rows start
  under their code, breaking where nvim breaks.

The pages in [docs](docs/README.md) list every key and file, and explain how
the screens hand off and how the cache works.

## Checks

```bash
make init   # once, installs golangci-lint
make check
```
