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
installation they inspect the saved profile. Existing installer state without a
profile is treated as `full` and saved during the next mutating command. A
profile-less configuration without installer state must be repaired by adding
`"profile": "full"` or `"profile": "terminal"`.

### `pde-installer update [--dry-run]`

Updates tools and home configuration for the saved profile. Use it after changes
to package lists, tool versions, runtimes, or local builds. It reconciles
managed components and applies managed home configuration. It requires saved
profile state, has no profile selector, and rejects UID 0.

### `pde-installer config [--dry-run]`

Applies managed home configuration for the saved profile without updating tools,
runtimes, packages, or local builds. Use it after changes only to shell, Git,
editor, or AI configuration. A normal run can update managed configuration files
and run source-managed scripts. It migrates older PDE settings automatically,
requires an installed Aqua-managed chezmoi binary, and rejects UID 0. Its dry
run uses read-only chezmoi status and diff without refreshing external content
or running scripts.

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
