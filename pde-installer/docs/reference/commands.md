# Command Reference

## Checkout Selection

Every command needs the repository root. Resolution order is:

1. `--repo-root PATH`
2. `PDE_REPO_ROOT`
3. the current directory and its parents

A valid root contains `chezmoi/`, `planner/go.mod`, and
`pde-installer/go.mod`.

## Commands

### `pde-installer install [--dry-run]`

Reconciles every managed component. It rejects UID 0. `--dry-run` prints
ordered actions and does not perform mutations.

### `pde-installer update [--dry-run]`

Runs the same full reconciliation as `install`. It has no tool or backend
selector.

### `pde-installer config [--dry-run]`

Migrates legacy PDE configuration and applies only the complete chezmoi source.
It rejects UID 0. Dry run executes read-only chezmoi status and diff without
refreshing externals or running scripts. A managed Aqua installation of chezmoi
must already exist.

### `pde-installer doctor`

Checks the non-root user, C and C++ compilation, build and archive commands, a
fetcher, Ubuntu and package-manager requirements, repository metadata, and
writable managed destinations. It exits with an error if any check fails.

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
