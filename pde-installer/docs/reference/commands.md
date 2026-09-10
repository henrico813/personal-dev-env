# Command Reference

## Checkout Selection

Every command needs the repository root. Resolution order is:

1. `--repo-root PATH`
2. `PDE_REPO_ROOT`
3. the current directory and its parents

A valid root contains `chezmoi/`, `planner/go.mod`, and
`pde-installer/go.mod`.

## Commands

### `pde-installer install [--profile full|terminal] [--dry-run]`

Reconciles the selected profile. On a fresh HOME, an install without a profile
defaults to `full`; an existing install reuses its saved profile. The selection is saved in `~/.config/pde/config.json`. A saved `terminal` profile
can expand to `full`, but a saved `full` profile cannot change to `terminal`
because installed components are not removed. It rejects UID 0. `--dry-run`
prints ordered actions and does not perform mutations.

Only `install` accepts `--profile`. `update` and `config` require a saved
profile and use it. On a fresh home, `doctor` and `list` inspect `full`; after
installation they inspect the saved profile. A legacy profile-less configuration must be repaired by adding `"profile": "full"` or `"profile": "terminal"` to
`~/.config/pde/config.json` before commands that require saved state can run.

### `pde-installer update [--dry-run]`

Reconciles the saved profile to repository pins. It requires saved profile
state and has no profile selector.

### `pde-installer config [--dry-run]`

Migrates legacy PDE configuration, then applies the chezmoi source for the
saved profile without reconciling other backends. It rejects UID 0. Dry run
executes read-only chezmoi status and diff without refreshing externals or
running scripts. A managed Aqua installation of chezmoi must already exist.

`config` also configures Git's template directory at
`~/.config/git/template`. Future `git init` and `git clone` operations receive
its `commit-msg` checker. To add it to an existing repository without a
`commit-msg` hook, run:

```bash
repository=/path/to/repository
hook="$(git -C "$repository" rev-parse --path-format=absolute --git-path hooks/commit-msg)"
test ! -e "$hook"
git -C "$repository" init --template="$HOME/.config/git/template"
```

### `pde-installer doctor`

Checks the non-root user, common archive commands and a fetcher, Ubuntu and
package-manager requirements, repository metadata, and writable managed
destinations for the resolved profile. C and C++ compilation and build-command
checks are full-profile-only; common archive and fetch checks apply to both
profiles. It exits with an error if any applicable check
fails.

### `pde-installer list`

Prints tab-separated columns:

```text
OWNER  ITEM  REQUESTED  INSTALLED  STATUS
```

Status words vary by owner. Common values include `missing`, `installed`,
`current`, `outdated`, `drifted`, and `unavailable`.

## Global Option

`--repo-root PATH` selects a checkout instead of using the environment or
current directory.
