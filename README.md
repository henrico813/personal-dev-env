# Personal Dev Environment

This repo now contains both the PDE installer and the Claude/OpenCode config that used to live in a separate repository.

## PDE Quick Start

Run the PDE installer directly from the `pde/` subtree:

```bash
./pde/pde minimal
./pde/pde full
```

See [`pde/README.md`](./pde/README.md) for the full PDE install flow, supported profiles, and test commands.

## Claude, OpenCode, and Codex Quick Start

Install the Claude config from the repo root:

```bash
./install.sh
```

The install script preserves existing Claude user data such as `projects/`, `.credentials.json`, and `history.jsonl`. It also installs the OpenCode-compatible commands and agents into `~/.config/opencode/`, and it installs the repo-managed Codex skills into `~/.codex/skills/`.

## Included Config

- `pde/` installs the shell, editor, tmux, fonts, and AI tooling used by PDE.
- `.claude/` contains Claude Code commands, hooks, agents, and helper scripts.
- `.opencode/` contains the OpenCode-compatible command and agent set.
- `.codex/` contains the Codex skill set for the active workflow commands.

## Requirements

- Linux system with `sudo` access for PDE installs.
- `jq` is required for the Claude status line and is installed by `./install.sh` on Linux.
