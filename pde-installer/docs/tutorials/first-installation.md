# First Installation

This tutorial builds the installer and reconciles one user account.

## 1. Check the Host

Use Ubuntu 22.04 or newer on Linux amd64. Sign in as a regular user
that can run `sudo apt-get`. You need Go 1.21 or newer to build the installer.

Clone the repository before starting. The installer does not clone it.

## 2. Build the Installer

Run from the repository root:

```bash
mkdir -p ~/.local/bin
go build -C pde-installer -o ~/.local/bin/pde-installer .
export PATH="$HOME/.local/bin:$PATH"
```

## 3. Preview the Work

For a terminal installation, preview the reduced profile:

```bash
pde-installer install --profile terminal --dry-run
```

The preview reads host and repository state but does not run mutating commands.
Omit `--profile terminal` to preview the full profile.

## 4. Install

Choose one profile:

```bash
pde-installer install --profile terminal
pde-installer install                 # full; also the default when omitted
```

Terminal installs the reduced Ubuntu prerequisites, terminal tools, and
terminal shell/tmux chezmoi configuration. It does not install the full
profile's runtimes, editor, LSP, npm/AI tools, fonts, or editor/AI
configuration. The installer saves the choice in
`~/.config/pde/config.json`; later `update`, `config`, `doctor`, and `list` use
that saved profile. A terminal installation can later expand to full.

## 5. Verify

```bash
pde-installer doctor
pde-installer list
```

`doctor` reports host or repository problems. `list` prints each inventory
item, its owner, requested version, installed version, and status.

See [commands](../reference/commands.md) for checkout selection and all options.
