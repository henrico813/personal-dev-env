# Chezmoi Source

This directory owns PDE home configuration and checksummed external content.
Every `pde-installer install` applies it during reconciliation. The installer
passes `PDE_PROFILE=full|terminal` and `PDE_COLOR_PROFILE` while rendering it;
apply through the installer so component and visual selections match.

Edit files here using chezmoi source names. Add remote archives to
`.chezmoiexternal.toml.tmpl` with a SHA-256 checksum.
`.chezmoidata.json` owns shared terminal palettes and application theme names;
templates should consume those values instead of embedding theme colors.

Preview and apply configuration changes from `pde-installer/`:

```bash
go run . install --dry-run --repo-root ..
go run . install --repo-root ..
```

The installer snapshots changed targets before apply. A failed apply restores
the snapshot. Modifier scripts preserve selected user-owned settings. A JSONC
modify-template preserves parsed values but may normalize formatting and remove
comments when it emits the merged file.

## GitHub Pull Request Reviews

The terminal and full profiles install `gh` and `delta`. Git uses delta as its
pager with line numbers and file navigation. Press `n` or `N` to move between
files, `/` to search, and `q` to quit.

Inspect the pull request context before reading its patch:

```bash
gh pr view 123
gh pr checks 123
gh pr diff 123 --name-only
```

Use `prd` with a pull request number, URL, branch, or no argument for the
current branch's pull request. Additional `gh pr diff` flags pass through:

```bash
prd 123
prd https://github.com/OWNER/REPO/pull/123
prd 123 --exclude 'generated/*'
```

For side-by-side output on a wide terminal, invoke the underlying tools:

```bash
gh pr diff 123 --color=never |
  delta --side-by-side --line-numbers --navigate
```

Check out a pull request when review requires repository search or tests. Use
the merge base to isolate changes introduced by its branch:

```bash
gh pr checkout 123
base="$(gh pr view 123 --json baseRefName --jq .baseRefName)"
git fetch origin "$base"
git diff "origin/$base"...HEAD
```

Delta only displays changes. Submit the overall review with `gh pr review`, or
use `gh pr diff 123 --web` when line-specific comments are needed.

## Terminal Workspaces

Use tmux as the attachment and recovery layer, with separate presentation
sessions so phone-sized tmux geometry does not resize the desktop session. Start
or resume the environment for the current directory with one command:

```bash
tm
```

`tm` requires Herdr on `PATH`; the full profile installs it. The command creates
`hub` and `pocket` sessions containing Herdr and an empty shell, plus a `dash`
session containing the four-pane `tw` layout. It attaches `pocket` at 72 columns
and below and `hub` at larger widths. Use `tm hub`, `tm pocket`, or `tm dash` to
select a view explicitly. All three sessions record the project root and use
names derived from its absolute path, so environments for multiple directories
can coexist. Hub and Pocket both attach Herdr's `default` session, so they expose
the same agents. Herdr owns those shared agent panes and their PTY dimensions;
the separate tmux sessions isolate only the surrounding presentation geometry.

Plain `tw DIR` remains available for adding another project window with left,
top, bottom, and right panes, using positional commands or the project's
`left`, `top`, `bottom`, and `right` keys. It still reads project `.tw.yml`, so
inspect that file before trusting it. Herdr's saved-machine catalog remains
user-owned state.

Use `Ctrl-A` for tmux and `Ctrl-B` for Herdr. Herdr uses its mobile status header
and full-screen machine switcher at 72 columns and below. Machine labels
distinguish Local and Wallace agents. The managed config is validated with Herdr
0.9.1, so run `herdr config check` after upgrades.

See the [maintenance guide](../pde-installer/docs/how-to/update-chezmoi-content.md).

## Agent GitHub approvals

Keep the normal `gh` login; no token setup is needed. Full-profile installs build `pde-gh-write` and its `pde-pr-approve` alias. The writer waits up to 90 seconds; requests expire after 4 hours. Approve from the Herdr popup, the Moshi phone, or `pde-pr-approve REQUEST_ID` in a real terminal. An answer within 90 seconds lets the waiting command finish with the result. If the command already exited 3, a detached runner performs the write once it is approved; rerun the identical command to read the result. See the [approval explanation](docs/explanation/pr-write-approval.md), [how-to guide](docs/how-to/pr-write-approval.md), and [reference](docs/reference/pr-write-approval.md).
