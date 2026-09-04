# PDE Installer

`pde-installer` installs and updates the PDE environment from an existing
`personal-dev-env` checkout.

## Requirements

- Ubuntu 22.04 or newer
- Linux on amd64 or arm64
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
pde-installer install --profile full
pde-installer doctor
pde-installer list
```

The installer does not clone or update the checkout. It finds the checkout from
the current directory, `--repo-root`, or `PDE_REPO_ROOT`.

## Profiles

The `terminal` profile installs the shell environment, tmux, common CLI tools,
Yazi, and configuration. Its Ubuntu packages are `ca-certificates`, `curl`,
`file`, `git`, `gzip`, `tar`, `unzip`, `xclip`, `xz-utils`, and `zsh`.

The `full` profile also installs languages, Neovim, language servers, Node
packages, AI tools, fonts, terminal emulator configuration, and programs built
from this repository.

Only `install` accepts `--profile`. `update`, `config`, `doctor`, and `list` use
the saved profile. See the [command reference](docs/reference/commands.md) for
profile rules and options.

## Documentation

- [Documentation index](docs/README.md)
- [First installation](docs/tutorials/first-installation.md)
- [Command reference](docs/reference/commands.md)
- [Recovery](docs/how-to/recover-an-installation.md)
- [Installation architecture](docs/explanation/installation-architecture.md)
