# PDE Installer

`pde-installer` reconciles the PDE environment from an existing
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
pde-installer install terminal --dry-run
pde-installer install terminal
pde-installer doctor
pde-installer list
```

The installer does not clone or update the checkout. It finds the checkout from
the current directory, `--repo-root`, or `PDE_REPO_ROOT`.

A fresh installation must run `install terminal` or `install full`. The
selection is saved in `~/.config/pde/config.json`. Later bare `install` runs
reuse the saved selection or infer full from `paths.env` or an `install_path`
field. A successful explicit selection switches future reconciliation; a
pre-commit failure preserves the prior selection. Switching from full to
terminal does not uninstall full-only artifacts. Terminal contains the runtime Ubuntu prerequisites
(`ca-certificates`, `curl`, `file`, `git`, `gzip`, `tar`, `unzip`, `xclip`,
`xz-utils`, and `zsh`), the existing amd64-only direct tmux 3.7b binary, fd,
fzf, ripgrep, bat, jq, chezmoi, eza, zoxide, bottom, yq, Yazi/ya, and
shell/tmux configuration, the retained bottom/Aqua configuration, common
plugin externals, Git template configuration, and PDE Git configuration.

Terminal excludes runtimes, Neovim, LSPs, npm/AI tools, fonts, Keychain, local
builds, Alacritty, WezTerm, and editor/AI configuration. Full retains all of
these components and everything else in the environment. Every install
reconciles both tools and managed home configuration. Fresh `doctor` and `list`
inspect full.

## Documentation

- [Documentation index](docs/README.md)
- [First installation](docs/tutorials/first-installation.md)
- [Command reference](docs/reference/commands.md)
- [Recovery](docs/how-to/recover-an-installation.md)
- [Installation architecture](docs/explanation/installation-architecture.md)
