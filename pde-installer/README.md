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
`behavior-focused-testing`, `delegate`, and `status-updates`, and its
OpenCode config leaves out
the plan and design commands and the plan reviewer agents. Client removes those
files and full's other skills if a full install left them. It still excludes Go,
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

## Desktops

On Ubuntu 24.04 or newer, `install i3` or `install sway` adds a desktop to a
machine that already has a saved profile:

```bash
pde-installer install full
pde-installer install i3
```

The desktop's packages come from apt, its wallpaper player is built and
installed in `~/.local/bin`, and its configs and scripts come from chezmoi.
Machines with no desktop get none of these files. The installer writes no login
entry; log in with the `i3` or `sway` session that Ubuntu's packages add.

The wallpaper plays the first `.mp4` file in `~/.config/wallpapers/`, or the
first `.webm` file when there is no `.mp4`. Copy a video there yourself. Without
one, the desktop shows Ubuntu's default image.

Settings for one machine go in `~/.config/i3/local.conf`: the monitor names,
which workspace goes on which monitor, the monitor layout command and the
terminal key. The installer writes that file once and never changes it.
`~/.config/gtk-3.0/settings.ini` is also written once, so a theme picked in
lxappearance stays. Edits to the other desktop files are lost on the next
install.

To join a Wi-Fi network, press Super+n or click the Wi-Fi status in polybar,
then pick a network in rofi. A new network asks for its password.

To leave notifications to dunst, the installer hides xfce4-notifyd's autostart
entry. That entry applies to every session, so the plain Xubuntu (Xfce) session
on a desktop machine shows no notifications.

To show clocks for other time zones in polybar, create
`~/.config/pde-desktop/world-clocks` yourself; the installer never writes it.
Each line is a short label, a time zone from `timedatectl list-timezones` (or
`-` for no time), and an optional [wttr.in](https://wttr.in) location for the
weather. Lines starting with `#` are skipped:

```text
# LABEL ZONE PLACE
AAA Etc/UTC Place+Name
BBB - Other+Place
```

Without the file, polybar shows only the local date, time and weather. The
local weather and each place's weather are fetched from wttr.in every minute.

On an NVIDIA GPU, use i3. Ubuntu 24.04 and 26.04 ship Sway versions older than
1.12, which start on NVIDIA's driver only with `--unsupported-gpu`. Passing that
flag needs a login entry that only root can install.

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
