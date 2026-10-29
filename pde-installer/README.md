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
pde-installer install terminal --color-profile everforest-dark --dry-run
pde-installer install terminal --color-profile everforest-dark
pde-installer doctor
pde-installer list
```

The installer does not clone or update the checkout. It finds the checkout from
the current directory, `--repo-root`, or `PDE_REPO_ROOT`.

Pick the profile for the machine:

- `terminal`: headless homelab VMs that run services. Shell, tmux, and
  command-line tools.
- `client`: a laptop that edits files, runs OpenCode, and shows herdr sessions
  from other machines, with nothing compiled by the installer. Full without Go,
  Rust, local builds other than `skill-lessons`, the PR approval tools, or most
  skills. Client installs no Go itself, so the machine still needs Go 1.21 or
  newer to build the installer, for example from `sudo apt install golang-go` on
  Ubuntu 24.04.
- `full`: development machines. Everything.

A fresh installation must run `install terminal`, `install client`, or
`install full`. The selection is saved in `~/.config/pde/config.json`. Later
bare `install` runs reuse the saved selection or infer full from `paths.env` or
an `install_path` field. A successful explicit selection switches future
reconciliation; a pre-commit failure preserves the prior selection. Switching
from full to terminal does not uninstall full-only artifacts. Client refuses to
install while `~/.local/share/pde/releases` holds release tools it leaves out,
such as Go from a past full install, because client replaces that folder without
them. Terminal contains the runtime Ubuntu prerequisites (`ca-certificates`,
`curl`, `file`, `git`, `gzip`, `tar`, `unzip`, `xclip`, `xz-utils`, and `zsh`),
the existing amd64-only direct tmux 3.7b binary, fd, fzf, ripgrep, bat, jq,
chezmoi, eza, zoxide, bottom, yq, Yazi/ya, and shell/tmux configuration, the
retained bottom/Aqua configuration, common plugin externals, Git template
configuration, and PDE Git configuration.

Terminal excludes runtimes, Neovim, LSPs, npm/AI tools, fonts, Keychain, local
builds, Alacritty, WezTerm, and editor/AI configuration. Client adds
`fontconfig`, `python3`, the managed fonts, Neovim and its plugins, Node, the
npm tools, herdr, nobs, `skill-lessons`, and the Alacritty, WezTerm, herdr,
Neovim, and OpenCode configuration with the shared agent guidance. Its skills
are only `obsidian-zettel`, `obsidian-daily-note`, `plain-direct-english`,
`behavior-focused-testing`, and `delegate`, and its OpenCode config leaves out
the plan and design commands and the plan reviewer agents. It still excludes Go,
Rust, LSPs, Keychain, Moshi Hook, the other local builds such as planner and
vibe, the native blink.cmp matcher, the herdr plugins, the `gh` approval guard,
and the Claude Code, Codex, and Pi configuration. Full retains all of these
components and everything else in the environment. Every install reconciles both
tools and managed home configuration. Fresh `doctor` and `list` inspect full.

The independent color selection accepts `tokyo-night`, `everforest-dark`, or
`gruvbox-dark`. Missing state defaults to Tokyo Night. All installs coordinate
tmux, Powerlevel10k, fzf, and bat. Client and full installs also coordinate
Alacritty, WezTerm, Neovim, and OpenCode; terminal installs retain those files
unchanged. Static rendering never recolors the local terminal during SSH
sessions. Restart OpenCode after changing profiles.

## Documentation

- [Documentation index](docs/README.md)
- [First installation](docs/tutorials/first-installation.md)
- [Command reference](docs/reference/commands.md)
- [Recovery](docs/how-to/recover-an-installation.md)
- [Installation architecture](docs/explanation/installation-architecture.md)

## OpenCode Memory Cap

On a client or full host with a running systemd user manager, PDE replaces the
npm `opencode` symlink with a launcher. Root OpenCode launches run in
`opencode.slice` with `MemoryMax=50%` and `MemorySwapMax=512M`; nested OpenCode
launches remain in that scope. Set `PDE_OPENCODE_MEMORY_MAX` or
`PDE_OPENCODE_MEMORY_SWAP_MAX` to change either setting. Set
`PDE_OPENCODE_MEMORY_MAX=off` to run uncapped. The launcher warns and runs
uncapped when no systemd user manager is available.

Restart all OpenCode TUIs and `opencode serve --service` after installation.
Existing processes keep their previous cgroup. PDE does not set `MemoryHigh`.
