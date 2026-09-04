# Command Reference

## Checkout Selection

Every command needs the repository root. Resolution order is:

1. `--repo-root PATH`
2. `PDE_REPO_ROOT`
3. the current directory and its parents

A valid root contains `chezmoi/`, `planner/go.mod`, and
`pde-installer/go.mod`.

## Commands

### `pde-installer install [--profile terminal|full] [--dry-run]`

Reconciles the selected profile. A successful install saves the profile in
`~/.config/pde/config.json`. A dry run prints ordered actions but does not make
changes or save the profile. The command rejects UID 0.

A saved `terminal` profile can expand to `full`. A saved `full` profile cannot
change to `terminal`. If the first terminal installation is interrupted, recover
the installation and rerun the same explicit command:

```bash
pde-installer install --profile terminal
```

### `pde-installer update [--dry-run]`

Reconciles the components in the saved profile. It does not accept a profile
override.

### `pde-installer config [--dry-run]`

Migrates legacy PDE configuration, then applies the chezmoi files for the saved
profile without reconciling other components. It does not accept a profile
override. Dry run executes read-only chezmoi status and diff without refreshing
externals or running scripts. A managed Aqua installation of chezmoi must
already exist. The command rejects UID 0.

### `pde-installer doctor`

Runs checks for the saved profile. The `full` profile also checks compilers and
build tools. It exits with an error if any check fails.

### `pde-installer list`

Prints tab-separated columns:

```text
OWNER  ITEM  REQUESTED  INSTALLED  STATUS
```

Status words vary by owner. Common values include `missing`, `installed`,
`current`, `outdated`, `drifted`, and `unavailable`.

`list` reports components in the saved profile.

## Profile Selection

Only `install` accepts `--profile`. Without it, commands use the saved profile.
A fresh system with neither `~/.config/pde/config.json` nor legacy `paths.env`
defaults to `full`.

If a nonempty `config.json` or legacy `paths.env` exists without a profile, the
command stops. Add `"profile": "full"` or `"profile": "terminal"` to
`~/.config/pde/config.json`, then rerun the command. This state does not default
to `full`.

## Global Option

`--repo-root PATH` selects a checkout instead of using the environment or
current directory.
