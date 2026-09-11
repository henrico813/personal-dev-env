# Command Reference

## Checkout Selection

Every command needs the repository root. Resolution order is:

1. `--repo-root PATH`
2. `PDE_REPO_ROOT`
3. the current directory and its parents

A valid root contains `chezmoi/`, `planner/go.mod`, and
`pde-installer/go.mod`.

## Commands

### `pde-installer install [terminal|full] [--dry-run]`

Reconciles tools and managed home configuration for one selection. A fresh HOME
must specify `terminal` or `full`; bare `install` exits with an error when no
saved or legacy selection exists. An explicit selection is saved in
`~/.config/pde/config.json`. Bare `install` reuses the saved selection.
Existing `paths.env` state or a config containing `install_path` without
`profile` is legacy full state and is saved during the next install.

A successful explicit argument may switch either direction; a pre-commit
failure preserves the prior selection. Switching from full to terminal stops
reconciling full-only components but does not uninstall existing full-only
artifacts. Switching from terminal to full adds the full inventory.

Every install reconciles packages, tools, runtimes, local builds, and managed
home configuration in dependency order. It migrates older PDE settings,
configures Git and chezmoi-managed content, and runs source-managed scripts.
It rejects UID 0. `--dry-run` prints ordered actions and does not mutate HOME.

The removed `update`, `config`, and `--profile` interfaces have no aliases.

Installation also configures Git's template directory at
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
