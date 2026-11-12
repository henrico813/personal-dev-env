# Install on Windows with WSL

This guide installs pdev inside Ubuntu 24.04 on WSL 2, the Linux system that
runs inside Windows. It needs Windows 10 or 11 on an x64 processor. Windows on
Arm does not work yet, because the installer's tmux is x64 only.

## On Windows

1. Open PowerShell as administrator and run:

   ```powershell
   wsl --install -d Ubuntu-24.04
   ```

   Restart Windows if it asks. Ubuntu then opens and asks for a new user name
   and password. Use this user for every later step. `sudo` asks for this
   password.

2. Open Windows Terminal. Windows 11 has it. On Windows 10, install it from
   the Microsoft Store.

3. Install the JetBrainsMono Nerd Font on Windows. Windows Terminal draws with
   Windows fonts and cannot see the copy pdev installs inside Ubuntu, so
   without it the prompt and editor icons show as boxes.
   1. Download `JetBrainsMono.zip` from the
      [Nerd Fonts releases](https://github.com/ryanoasis/nerd-fonts/releases/latest).
   2. Unzip it, select the `.ttf` files, right-click them and choose Install.
   3. In Windows Terminal, open Settings, choose the Ubuntu-24.04 profile, then
      Appearance, and set Font face to `JetBrainsMono Nerd Font`.

## In Ubuntu

Open an Ubuntu-24.04 tab in Windows Terminal for these steps.

1. Keep the repository on Ubuntu's own disk, not on a Windows drive. A
   checkout there is slow, and the install script refuses it. For a new
   checkout:

   ```bash
   mkdir -p ~/Projects
   cd ~/Projects
   git clone https://github.com/henrico813/personal-dev-env.git
   cd personal-dev-env
   ```

   For a checkout you already have:

   ```bash
   cd ~/Projects/personal-dev-env
   git pull --ff-only
   ```

2. For `client` or `full`, set your Git email once:

   ```bash
   git config --global user.email you@example.com
   ```

3. Run the install script with one profile:

   ```bash
   bash install/wsl.sh full
   ```

   `terminal` installs the shell, tmux and command-line tools. `client` adds
   Neovim, OpenCode and Herdr and compiles nothing. `full` adds Go, Rust and
   the local builds for a development machine. The script asks for your
   Ubuntu password to install packages and to make zsh your login shell.

4. Read the last line. `pdev install ok` means the install works. Close the tab
   and open a new one to start zsh. Otherwise the lines above it say what
   failed or is missing. Fix them and run the script again, which is safe
   to repeat.

## What works differently in WSL

- `terminal`, `client` and `full` work. The `i3` and `sway` desktops need a
  Linux desktop, which WSL lacks, so the installer refuses them.
