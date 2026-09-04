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

```bash
pde-installer install --dry-run
```

The preview reads host and repository state but does not run mutating commands.

## 4. Install

```bash
pde-installer install
```

Enter your sudo password if `apt-get` asks for it. The command installs missing
Ubuntu dependencies, then reconciles every user-owned component and the chezmoi
configuration. You cannot select individual tools or backends.

## 5. Verify

```bash
pde-installer doctor
pde-installer list
```

`doctor` reports host or repository problems. `list` prints each inventory
item, its owner, requested version, installed version, and status.

See [commands](../reference/commands.md) for checkout selection and all options.
