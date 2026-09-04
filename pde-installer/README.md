# PDE Installer

`pde-installer` installs and updates the PDE environment from an existing
`personal-dev-env` checkout.

## Requirements

- Ubuntu 22.04 or newer
- Linux on amd64
- An unprivileged user with `sudo` access for `apt-get`
- Go 1.21 or newer to build the installer

Do not run mutating commands as root. The installer uses `sudo apt-get` only
for missing operating-system dependencies. Other managed files stay under
`HOME`.

## Quick Start

From the repository root:

```bash
mkdir -p ~/.local/bin
go build -C pde-installer -o ~/.local/bin/pde-installer .
export PATH="$HOME/.local/bin:$PATH"
pde-installer install --dry-run
pde-installer install
pde-installer doctor
pde-installer list
```

The installer does not clone or update the checkout. It finds the checkout from
the current directory, `--repo-root`, or `PDE_REPO_ROOT`.

`install` and `update` run the same full reconciliation. They do not accept a
tool or backend selector. `config` is the only supported subset; it migrates
legacy state and applies the repository's chezmoi content.

## Documentation

- [Documentation index](docs/README.md)
- [First installation](docs/tutorials/first-installation.md)
- [Command reference](docs/reference/commands.md)
- [Recovery](docs/how-to/recover-an-installation.md)
- [Installation architecture](docs/explanation/installation-architecture.md)
