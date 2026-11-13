# hub

`hub` is the command a hub runs. A hub is an AI agent session in
Herdr that starts other sessions and keeps track of them. `hub`
watches those sessions, sends them messages, reads their reports and
closes their tabs when they finish.

`pde-installer install full` builds this folder with cargo and
installs it as `~/.local/bin/hub`. Client laptops, which compile
nothing, do not get it.

To learn to use it, read [the documentation](docs/README.md).

## Build and check

```bash
make build   # target/release/hub
make check   # format, lint and test, as CI does
make test
```

## Versions and releases

Each release has a git tag `hub-vX.Y.Z`, and the newest tag sets the
version that `hub --version` prints. Before the first tag, it prints
the version in `Cargo.toml`. CI makes a new release, with a Linux
binary, when a merged change to this folder adds a feature, a fix, a
speed-up or a breaking change.

## Screens agents can see and tap

Every full-screen view the hub draws, such as `hub board`, follows
these rules, so an agent sees and taps exactly what the user does.

- The screen works out a list of the page's elements from the area
  and scroll of its last draw. Each has an `id` that stays the same
  across reads, such as `card:DEMO-KB-007`, a `kind`, its `text` and
  `hidden`, and `tap` when a tap on it does something. `kind` is
  `count`, `heading`, `item`, `more`, `back` or `text`. A drawn
  element also has its `column`, `row` and `width`, and `chosen` is
  true on the row Enter acts on.
- The list holds everything on the page, not only what fits. Rows
  scrolled off, the cards of a folded group and the cards behind
  `+ N more` have `hidden` true, `hidden_by` (`scroll`, `fold` or
  `more`) and `show`, the step to send next, such as
  `tap group:ready` or `press pagedown`. An agent sends `show` and
  finds the element in the reply until `hidden` is false. A far row
  can take many sends, and `show` can change on the way, such as
  from `press pagedown` to `tap group:ready`. The cards of a folded
  group or behind `+ N more` come right after that line.
- A tap finds the drawn element under it in that list. The live
  screen, `--send` and `--control` use the same lookup. `tap ID` on a
  hidden element fails and names its `show` step.
- The screen offers `--dump`, which prints the list as JSON once,
  `--listen` and `--send`, which let an agent tap the screen the user
  watches and print the list and the screen's text after each step,
  and `--control`, which does the same without a terminal for tests.
- Marks and labels a tap can hit are plain ASCII, so each takes one
  column in every terminal and a tap's column is exact.
