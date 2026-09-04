# PDE

PDE configuration is a chezmoi source in `chezmoi/`. Installation is owned by
the Go application in `pde-installer/`; the separate `pde` command manages vault
configuration and lookup only.

## Quick Start

Install the host compiler, development headers, `yacc`, archive tools, Git, and
a fetcher first. The installer itself is rootless and never invokes a system
package manager or `sudo`.

```bash
mkdir -p ~/.local/bin
go build -C pde-installer -o ~/.local/bin/pde-installer .
pde-installer install
```

Run from the checkout, pass `--repo-root`, or set `PDE_REPO_ROOT`.

## Commands

```bash
pde-installer install
pde-installer update
pde-installer doctor
pde-installer list
pde-installer config
```

- `install` and `update` reconcile the same pinned desired state.
- `doctor` checks host prerequisites, repository data, and managed paths.
- `list` reports each item, owner, requested version, installed version, and status.
- `config` applies only the repository chezmoi source.
- `--dry-run` on install and update prints ordered work without changing `HOME`.
- `config --dry-run` executes read-only chezmoi status and diff without refreshing externals or scripts.

All managed destinations are below `HOME`: pkgsrc source is
`~/.local/src/pkgsrc-2026Q2`, its unprivileged prefix is `~/.local/pkg`, Aqua is
`~/.local/share/aquaproj-aqua`, direct releases are in
`~/.local/share/pde/releases`, and launchers are in `~/.local/bin`.

pkgsrc is bootstrapped from its pinned source archive. Neovim, Go, Rust,
Node.js, and Keychain use checksum-verified upstream releases instead of source
builds. Aqua tools, npm packages, fonts, Neovim plugins, shell plugins, tmux
plugins, and local repository builds are pinned or input-hashed. Chezmoi owns
home configuration and preserves local fields handled by its modifier scripts.
Shell and tmux plugins load only from checksummed local external directories.

`~/.config/pde/config.json` remains the source of truth for vault and OpenCode
settings. All tracked AI agents, commands, skills, and settings are applied by
chezmoi from repository-local, checksummed file externals.

## Testing

```bash
go test -C pde-installer ./...
go test -race -C pde-installer ./...
go vet -C pde-installer ./...
./pde-installer/test/run-tests.sh smoke
```

Set `PDE_TEST_OFFICIAL_TOOLS=1` when running the direct package tests to
download and verify the pinned Neovim, Go, Rust, Node.js, and Keychain releases.

The Go tests exercise real installer code with temporary files and small local
executables. They verify pkgsrc decisions, npm and local-build activation,
chezmoi rollback, installer locking, and recovery after an interrupted change.
They stay fast because they do not download or compile complete toolchains.

The Docker smoke runs all five commands as an unprivileged user on Ubuntu 22.04
and 24.04 and confirms that root execution is rejected. Install and update
remain read-only dry runs. A complete installation is a manual real-world check.
