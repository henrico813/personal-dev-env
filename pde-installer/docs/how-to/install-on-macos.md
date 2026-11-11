# Install on macOS

This guide installs pdev on a Mac with Apple Silicon, such as an M1 or later.
Intel Macs do not work. One script installs everything and then checks it.

macOS gets two of the three profiles:

- `terminal` installs the shell, tmux and the command-line tools.
- `client` adds Neovim, OpenCode, Herdr and the fonts. The script uses
  `client` when you name no profile.

`full` and the `i3` and `sway` desktops need Linux, and the installer refuses
them on macOS.

## Before you start

1. Open Terminal and install Apple's command-line tools:

   ```bash
   xcode-select --install
   ```

2. Install Homebrew with the command from [brew.sh](https://brew.sh):

   ```bash
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```

3. For `client`, set your Git email once:

   ```bash
   git config --global user.email you@example.com
   ```

## Install

The script keeps the repository in `~/Projects/personal-dev-env`. It clones
the repository there when it is missing, and runs `git pull --ff-only` when it
is already there. Set `PDE_DIR` to use another folder.

For a new Mac, clone the repository and run the script:

```bash
mkdir -p ~/Projects
git clone https://github.com/henrico813/personal-dev-env.git ~/Projects/personal-dev-env
bash ~/Projects/personal-dev-env/install/macos.sh client
```

For a Mac that already has the checkout, run the script, which pulls first:

```bash
bash ~/Projects/personal-dev-env/install/macos.sh client
```

Use `terminal` in place of `client` for the smaller profile.

## Check the result

The last line says whether the install works:

- `pdev install ok` means every item is in place. Open a new Terminal window
  to start zsh with the new setup.
- `pdev install incomplete` means something is missing. The `error` lines from
  `pde-installer doctor` and the item lines from `pde-installer list` above it
  say what. Fix them and run the script again. Running it again is safe.

## What is different on macOS

- Homebrew installs git, tmux, fd, bat, eza and, for `client`, Python 3.13.
  The other tools come from the same pinned releases as on Linux.
- The fonts go to `~/Library/Fonts/pde`. Choose `JetBrainsMono Nerd Font` in
  your terminal's settings.
- `nobs` is not installed, because it has no macOS build.
- OpenCode runs without the memory cap that systemd gives it on Linux.
- tmux copies to the clipboard with `pbcopy`.
