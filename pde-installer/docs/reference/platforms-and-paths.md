# Platforms and Paths

## Supported Host

| Requirement | Value |
|---|---|
| Operating system | Ubuntu 22.04 or newer |
| Kernel platform | Linux |
| Architecture | amd64 or arm64 |
| User | Non-root; sudo access is required for missing apt packages |

Mutating commands reject UID 0. `doctor` also reports UID 0 as a problem.

## Managed Paths

| Path | Purpose |
|---|---|
| `~/.local/bin/` | Launchers and locally built applications |
| `~/.local/share/aquaproj-aqua/` | Aqua and Aqua packages |
| `~/.local/share/pde/releases/` | Direct-release tools and runtimes |
| `~/.local/share/pde/npm/` | npm package prefix |
| `~/.local/share/pde/tmux/<version>/` | Built tmux release |
| `~/.local/share/fonts/pde/` | Managed fonts |
| `~/.local/state/pde/` | Installer lock, journals, and build state |
| `~/.config/pde/config.json` | PDE install path and migrated vault settings |
| `~/.config/` and other home paths | Chezmoi-managed configuration |

The installer checks that managed destinations stay under `HOME`. Journal files
also stay under `HOME`; `XDG_STATE_HOME` does not move them. An absolute
`XDG_STATE_HOME` is passed only to chezmoi's Surveil state pattern.

The exception to user-local writes is apt. `sudo apt-get update` and
`sudo apt-get install` change system package state, and journals do not roll
those changes back. `fc-cache -f` refreshes the font cache outside rollback.
