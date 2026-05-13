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

`config` is a standalone, no-sudo shared-config migration helper. It links the managed PDE shell files into the home directory and preserves an existing `PDE_PROFILE` line, plus any existing `PDE_MAIN_VAULT` and `PDE_WORK_VAULT` entries in `~/.config/pde/paths.env`, when one is already present, but it does not install runtimes, plugins, or profile extras.

The `obsidian`, `vault`, and `ai-tools` commands are available through the Go CLI after `minimal` provides the PDE Neovim config:

```bash
cd cli && go build -o ~/.local/bin/pde .
pde install config
pde install obsidian
pde install ai-tools
```

`obsidian` still depends on `minimal` because it uses the PDE Neovim config.

`ai-tools` installs planner, `codex`, `opencode`, `opencode-inline-shim`, `pi`, `surveil`, and `vibe`, then copies the neutral `ai/` config tree into the user’s managed config paths. `surveil` and `vibe` install through Cargo, so `cargo` must already be available, and `vibe run` requires Docker plus provider auth via env vars or `~/.pi/agent/auth.json`.

`vault` exposes `pde vault default get` and `pde vault default set <main|work>` so the Go CLI owns the default-selector workflow. `pde vault default get` prints `main`, `work`, or `unset`, and `pde vault locate --vault default` follows that selector before the legacy fallback. It starts with `pde vault locate --json --vault default "<reference>"` and widens to `--vault any "<reference>"` only after a `not_found` result. Use `--query` only when you explicitly want note-content search.

`~/.config/pde/paths.env` is the source of truth for `OPENCODE_BASE_URL`, `OPENCODE_INLINE_SHIM_PORT`, `OPENCODE_INLINE_MODEL`, `PDE_MAIN_VAULT`, and `PDE_WORK_VAULT`; running `pde vault default set <main|work>` also persists `PDE_DEFAULT_VAULT` there. PDE shells export those variables automatically; Neovim only falls back to reading them from `paths.env` when it starts outside a PDE-managed shell. `<leader>pm` and `<leader>pM` intentionally share CodeCompanion's selector over OpenCode ACP models: chat applies to the active chat session, while inline stores a separate session override that flows through `opencode-inline-shim`. When there is no inline session override and no `OPENCODE_INLINE_MODEL`, the shim lets OpenCode choose its current default model. For the default loopback setup, `opencode-inline-shim` starts `opencode serve` on demand. Thinking-suffixed inline selections are accepted for compatibility with the shared selector, but the shim currently strips the suffix and sends only the base backend model until explicit OpenCode HTTP thinking support is verified. After changing any of those env values, restart `opencode-inline-shim` so the shim picks up the new environment.

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
