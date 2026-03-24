# PDE

PDE is the shell-based installer and config set that lives under `pde/` inside this repository.

## Quick Start

Run from the repo root:

```bash
./pde/pde minimal
./pde/pde full
```

Bootstrap directly onto a Linux machine:

```bash
curl -fsSL https://raw.githubusercontent.com/henrico813/personal-dev-env/main/pde/bootstrap.sh | bash -s -- minimal
```

## Profiles

- `minimal`: shell, tmux, Rust tooling, Neovim, and shared config files.
- `full`: everything in `minimal`, plus Node, Claude Code, fonts, and GUI-oriented extras.

## Repository Layout

- `pde/pde`: profile entrypoint.
- `pde/lib/`: installer modules split by concern.
- `pde/config/`: tracked zsh, tmux, Neovim, and terminal config.
- `pde/test/`: Docker-based installer tests and verification scripts.

## What It Installs

- Shell: `zsh`, antidote, powerlevel10k, tmux.
- Tools: `ripgrep`, `fzf`, `bat`, `jq`, `eza`, `zoxide`, `yazi`.
- Editor: Neovim with LazyVim plus tracked custom plugins.
- Full profile extras: Node via `nvm`, Claude Code, fonts, and Alacritty config.

## Testing

From `pde/`:

```bash
./test/run-tests.sh minimal
./test/run-tests.sh full
./test/run-tests.sh idempotent
```
