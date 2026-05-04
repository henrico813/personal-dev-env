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

## Optional Targets

Some setups belong beside the base profiles, not inside them.
`obsidian` and `ai-tools` are installed through the Go CLI after `minimal` provides the PDE Neovim config:

```bash
cd cli && go build -o ~/.local/bin/pde .
pde install obsidian
pde install ai-tools
```

`ai-tools` installs planner, `codex`, `opencode`, `opencode-inline-shim`, `pi`, and `vibe`, then copies the neutral `ai/` config tree into the user’s managed config paths. `vibe` installs through Cargo, so `cargo` must already be available, and `vibe run` requires Docker plus provider auth via env vars or `~/.pi/agent/auth.json`.

`~/.config/pde/paths.env` is the source of truth for `OPENCODE_BASE_URL` and `OPENCODE_INLINE_SHIM_PORT`. PDE shells export those variables automatically; Neovim only falls back to reading them from `paths.env` when it starts outside a PDE-managed shell. For the default loopback setup, `opencode-inline-shim` starts `opencode serve` on demand. After changing either value, restart `opencode-inline-shim` so the shim picks up the new environment.

## Repository Layout

- `pde/pde`: profile entrypoint.
- `pde/lib/`: installer modules split by concern.
- `pde/config/`: tracked zsh, tmux, Neovim, and terminal config.
- `pde/test/`: Docker-based installer tests and verification scripts.

## What It Installs

- Shell: `zsh`, antidote, powerlevel10k, tmux.
- Tools: `ripgrep`, `fzf`, `bat`, `jq`, `eza`, `zoxide`, `yazi`, `btm`.
- Editor: Neovim with the tracked PDE nvim config and plugins.
- Full profile extras: Node via `nvm`, Claude Code, fonts, and Alacritty config.

## Testing

From `pde/`:

```bash
./test/run-tests.sh minimal
./test/run-tests.sh full
./test/run-tests.sh idempotent
```
