# Chezmoi Source

This directory owns PDE home configuration and checksummed external content.
Every `pde-installer install` applies it during reconciliation. The installer
passes the profile's tiers as `.pde.tiers` with `--override-data`, and
`PDE_COLOR_PROFILE` in the environment; apply through the installer so
component and visual selections match. Templates test a tier, such as
`{{ if has "workstation" .pde.tiers }}`, instead of a profile name. The
profiles and their tiers are in `pde-installer/internal/profile/profiles.yaml`.
`.pde.desktop` holds the machine's desktop, `i3` or `sway`, and is empty
without one. `.chezmoiignore.tmpl` uses it to keep desktop files off other
machines.

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

Every profile installs `gh` and `delta`. Git uses delta as its pager with line
numbers and file navigation. Press `n` or `N` to move between files, `/` to
search, and `q` to quit.

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

On a phone, run `prs` to pick from the open pull requests that involve you in
any repository. The selected one's repository, state, review, and checks show
below the list, in color; set `NO_COLOR` to turn colors off. The keys follow
vim: `j` and `k` move, `g` and `G` jump to the first and last, `Ctrl-D` and
`Ctrl-U` move half a page, and `J` and `K` scroll the preview. Press `l` or
Enter, or double-tap, for the full view, which adds the files, description,
comments, reviews, and code threads. Open threads come first with their code;
resolved ones take one line each. Press `c` for only the comments, `d` for the
diff, `p` to hide the preview, `r` to reload, and `q` to quit. Press `/` to
search the list, and Esc to clear the search and get the keys back. `prs` only
reads from GitHub. To show one pull request without the list, give its number
inside its repository, or its repository and number from anywhere:

```bash
prs 123
prs OWNER/REPO 123
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

## Worktrees and Branch Names

Every profile installs Worktrunk (`wt`). It needs Git 2.43 or newer, so stock
Ubuntu 22.04, which ships Git 2.34, needs a newer Git first.

A worktree's folder is its branch with `/` replaced by `-`, under the
repository's `worktrees/` folder, so
`fix/abc-12-login-timeout` lives in `worktrees/fix-abc-12-login-timeout`. List
worktrees with their branches, or check out a pull request into its own folder:

```bash
wt list
wt switch pr:123
```

A branch for planned work starts with its plan's issue key, in lowercase:
plan `ABC-12` gets `feat/abc-12-approvals-file` and the folder
`worktrees/feat-abc-12-approvals-file`. Agents get the key from
`planner key "<plan file>"`, which prints `abc-12` and fails for a file with no
key, such as a design doc. Pull request titles leave the key
out, but the pull request's branch keeps it, so one key finds the plan, the
branch and its worktree, and the pull request:

```bash
nobs find abc-12
wt list | rg abc-12
gh pr list -s all -L 200 | rg abc-12
```

`wt switch` with no branch opens a picker that filters by branch, folder and
pull request number as you type, so typing `abc-12` there and pressing
Enter opens the worktree. A fix with no plan has no key; search for its branch
words, such as `login-timeout`, instead.

A new branch must be `<type>/<words>`: a commit type or `vibe`, then lowercase
letters and digits joined by hyphens. Existing branches and pull request
branches are not checked. A repository can replace the rule in its own Git
config with an extended regular expression, as used by `grep -E`:

```bash
git config pde.branchPattern '^[A-Z]+-[0-9]+-[a-z0-9-]+$'
```

Under a pattern, the branch still starts with the ticket key the plan
names, in the form the pattern asks for; agents ask the user when the
plan names none. Vibe keys and run folders keep using `planner key`.

`~/.zshenv` points Worktrunk at `~/.config/pde/worktrunk.toml`, and
`~/.config/worktrunk/config.toml` can override it. Do not use `wt merge` or
`wt remove`: changes merge through pull requests, and `wt remove` deletes
ignored files such as `.env` without asking. After each Worktrunk upgrade, check
in a fresh `zsh -c` that `wt switch --create docs/x` makes `worktrees/docs-x`
and `wt switch --create Bad_Name` is refused, since a renamed setting would
otherwise go unnoticed.

A new worktree starts with a copy of the main checkout's
`.code-comments.json`, your approved comments, when Git ignores that
file. Removing a worktree with the `cleanup-plan` skill brings its
approvals back to the main checkout.

## Terminal Workspaces

Use tmux as the attachment and recovery layer, with separate presentation
sessions so phone-sized tmux geometry does not resize the desktop session. Start
or resume the environment for the current directory with one command:

```bash
tm
```

`tm` requires Herdr on `PATH`; the client and full profiles install it. The
command creates `hub` and `pocket` sessions containing Herdr and an empty shell,
plus a `dash` session containing the four-pane `tw` layout. It attaches `pocket`
at 100 columns and below and `hub` at larger widths. Use `tm hub`, `tm pocket`,
or `tm dash` to select a view explicitly. All three sessions record the project
root and use names derived from its absolute path, so environments for multiple
directories can coexist. Hub and Pocket both attach Herdr's `default` session,
so they expose the same agents. Herdr owns those shared agent panes and their
PTY dimensions; the separate tmux sessions isolate only the surrounding
presentation geometry.

Plain `tw DIR` remains available for adding another project window with left,
top, bottom, and right panes, using positional commands or the project's
`left`, `top`, `bottom`, and `right` keys. It still reads project `.tw.yml`, so
inspect that file before trusting it. Herdr's saved-machine catalog remains
user-owned state.

Use `Ctrl-A` for tmux and `Ctrl-B` for Herdr. Herdr uses its mobile status header
and full-screen machine switcher at 100 columns and below. Machine labels
distinguish Local and Wallace agents. The managed config is validated with Herdr
0.9.1, so run `herdr config check` after upgrades.

See the [maintenance guide](../pde-installer/docs/how-to/update-chezmoi-content.md).

## Agent GitHub approvals

The full profile makes agents ask you before they write to a pull request; see
the [approval docs](../pdev2/docs/README.md).

## Comment review

The full profile binds Ctrl+b e, in place of Herdr's scrollback editor, to a
phone-sized Herdr popup for deciding on an AI's proposals for Go comments, and
Ctrl+b E to the same screen in a Herdr tab; see the
[comment review docs](../pde-comment-review/docs/README.md). Client machines
share that Herdr config but not the popup. There Ctrl+b e opens the comment
review of the machine Herdr shows, and fails on the laptop's own sessions.
