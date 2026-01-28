# Personal Development Environment

Ansible-based setup for a productive Ubuntu/Debian development environment.

## Quick Start

### Prerequisites
- Ubuntu 22.04+ or Debian 11+ (x86_64)
- Ansible 2.9+
- sudo access

### Full Installation
```bash
cd pde
make deploy-dev
```

### Server Installation (minimal)
```bash
make deploy-server HOST=myserver
```

## Profiles

| Profile | Fonts | Dev Tools | AI Tools | Use Case |
|---------|-------|-----------|----------|----------|
| development | Yes | Yes | Yes | Workstations, dev VMs |
| server | No | No | No | Proxmox hosts, NAS |
| minimal | No | No | No | Containers, minimal VMs |

## What Gets Installed

### System Packages (requires sudo)
- Shell: zsh, tmux, screen
- CLI Tools: fd-find, fzf, ripgrep, bat, trash-cli, zoxide, jq
- Development: git, python3, curl

### User Tools (no sudo)
- Rust: Latest stable via rustup
- Node.js: v20 via nvm
- Editors: Neovim with LazyVim
- Shell: antidote, powerlevel10k, eza
- AI Tools: aider, codex, Claude Code (development profile only)

## Neovim Plugin Customization

Custom Neovim plugins are managed in `pde/config/nvim/lua/plugins/`. These plugins are **always synced** to `~/.config/nvim/lua/plugins/` on every deploy, regardless of whether LazyVim is freshly installed or already exists.

This allows you to:
- Add new plugins by creating `.lua` files in `pde/config/nvim/lua/plugins/`
- Update plugin configurations and have them applied on next deploy
- Keep your custom plugins version-controlled in this repo

### Adding a Custom Plugin

1. Create a new file in `pde/config/nvim/lua/plugins/`:
   ```lua
   -- pde/config/nvim/lua/plugins/my-plugin.lua
   return {
     "author/plugin-name",
     opts = {
       -- plugin configuration
     },
   }
   ```

2. Deploy to sync: `./pde full` or `./pde minimal`

3. Open nvim - lazy.nvim will auto-install the new plugin

### Current Custom Plugins

| Plugin | Purpose |
|--------|---------|
| `llm.lua` | FIM autocomplete via local LLM backend |

## Architecture

This playbook uses a privilege-separated architecture:

- **system.yml** - System-level tasks requiring sudo (apt packages, locale, timezone, neovim)
- **user.yml** - User-level tasks without sudo (rust, shell config, dev tools)
- **site.yml** - Orchestrator that runs both playbooks

### Running Individually

```bash
# System setup only (requires sudo)
ansible-playbook system.yml --ask-become-pass

# User setup only (no sudo needed)
ansible-playbook user.yml

# Both (full install)
ansible-playbook site.yml --ask-become-pass
```

## Customization

### Override Defaults
Edit `defaults.yml` or pass via command line:
```bash
ansible-playbook site.yml -e "timezone=America/New_York node_version=22"
```

### Change Versions
Edit `vars/versions.yml` to pin specific versions.

## Makefile Targets

- `make deploy-dev` - Full development setup on localhost
- `make deploy-server HOST=name` - Minimal server setup
- `make deploy-shell-only HOST=name` - Just shell configuration
- `make list-hosts` - Show all inventory hosts

## Platform Support

- **Supported:** Ubuntu 22.04+, Debian 11+ (x86_64)
- **Not supported:** macOS, Fedora, Arch, ARM

## Files Overview

### Playbooks
| File | Purpose |
|------|---------|
| `site.yml` | Main orchestrator (use this) |
| `system.yml` | System-level setup (sudo required) |
| `user.yml` | User-level setup (no sudo) |
| `main.yml` | DEPRECATED - use site.yml (removal: 2026-06-01) |

### Task Files (new - no sudo)
| File | Purpose |
|------|---------|
| `tasks/install_rust.yml` | Rust + cargo-binstall |
| `tasks/install_shell_user.yml` | Shell setup (antidote, p10k, tpm, lazyvim) |
| `tasks/install_tools_user.yml` | Dev tools (nvm, node, yazi, AI tools) |
| `tasks/install_configs.yml` | Config file copying |

### Task Files (legacy - for backwards compatibility)

These files support the deprecated `main.yml` and will be removed after 2026-06-01.

| File | Purpose |
|------|---------|
| `tasks/install_deps_legacy.yml` | System dependencies |
| `tasks/install_shell_legacy.yml` | Shell setup |
| `tasks/install_tools_legacy.yml` | Dev tools |
| `tasks/install_fonts.yml` | Font installation (used by both) |

### Configuration
| File | Purpose |
|------|---------|
| `defaults.yml` | User-overrideable defaults |
| `vars/versions.yml` | Version pinning |
| `inventory.yml` | Host and profile definitions |
| `group_vars/*.yml` | Profile-specific variables |

## Dependency Guards

The playbooks include dependency guards to prevent partial installation failures:

- **user.yml** checks for system prerequisites (git, zsh, curl, unzip) and warns if missing
- **install_shell_user.yml** requires Rust/cargo to be installed first
- **npm-based tools** skip gracefully if NVM is not installed

If you see warnings about missing prerequisites, run `system.yml` first.
