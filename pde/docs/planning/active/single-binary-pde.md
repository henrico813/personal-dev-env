# Plan: Single Binary PDE

## Goal

Replace the Ansible-based system with a single bash script that has two modes:
- `pde minimal` - Full shell experience for any remote machine
- `pde full` - Everything including AI tools and fonts for GUI machines

## Design Principles

1. **One file to understand** - The main script is readable top-to-bottom
2. **Libs are optional reading** - Implementation details hidden in lib/
3. **Idempotent** - Safe to run multiple times
4. **Fast** - Skip work already done
5. **Loud failures** - Fail fast with clear errors, clean up on failure
6. **Visual consistency** - Same prompt, colors, keybindings everywhere

## Profile Definitions (Finalized)

Based on user requirements:

| Aspect | Minimal | Full |
|--------|---------|------|
| **Use case** | Any remote machine (SSH) | GUI machines (workstations) |
| **Typical tasks** | Quick edits, commands, logs | Extended development |
| **Visual feel** | Identical to full | Full experience |

## Profile Feature Matrix (Finalized)

| Feature | minimal | full | Notes |
|---------|:-------:|:----:|-------|
| **Shell** |
| zsh | yes | yes | |
| tmux 3.6a (source) | yes | yes | Modern features needed |
| p10k theme | yes | yes | Visual consistency |
| antidote plugins | yes | yes | |
| tpm (tmux plugins) | yes | yes | |
| **CLI Tools** |
| fd-find | yes | yes | apt |
| ripgrep | yes | yes | apt |
| bat | yes | yes | apt |
| fzf | yes | yes | apt |
| jq | yes | yes | apt |
| eza | yes | yes | cargo-binstall |
| zoxide | yes | yes | cargo-binstall |
| htop/btop | yes | yes | apt |
| unzip | yes | yes | apt (needed for yazi) |
| trash-cli | no | yes | apt |
| **Editors** |
| neovim | yes | yes | AppImage (arch-aware) |
| LazyVim | yes | yes | Full IDE everywhere |
| **File Manager** |
| yazi | yes | yes | Useful everywhere |
| **Runtimes** |
| rust + cargo-binstall | yes | yes | Needed for eza/zoxide |
| nvm + node | no | yes | Only for Claude |
| **AI Tools** |
| Claude Code | no | yes | npm install |
| aider | no | no | Not needed |
| codex | no | no | Not needed |
| **Desktop** |
| Nerd Fonts | no | yes | Only for GUI terminals |
| alacritty | no | yes | Terminal emulator |
| fontconfig | no | yes | apt (needed for fc-cache) |
| **Shell Functions** |
| tw() | yes | yes | Tmux helper, useful everywhere |
| ai() | no | yes | Claude launcher |
| **Legacy** |
| screen | no | no | Skipped entirely |

## Directory Structure

```
pde/
├── pde                     # The one binary (bash script)
├── bootstrap.sh            # Remote installation helper
├── lib/
│   ├── core.sh             # Shared helpers (log, has, install_apt, etc.)
│   ├── shell.sh            # zsh, tmux (from source), antidote, p10k, tpm
│   ├── tools.sh            # apt tools + cargo-binstall tools
│   ├── editor.sh           # neovim, lazyvim
│   ├── ai.sh               # nvm, node, claude
│   └── fonts.sh            # nerd fonts
├── config/
│   ├── zsh/
│   │   ├── zshrc           # Main config with tw(), conditional rm alias
│   │   ├── zshrc.ai        # ai() function only
│   │   └── zsh_plugins.txt
│   ├── tmux/
│   │   └── tmux.conf
│   ├── p10k/
│   │   └── p10k.zsh
│   └── alacritty/
│       └── alacritty.toml
├── Makefile                # Retained for `make claude` target only
└── docs/
```

## The Main Script

```bash
#!/usr/bin/env bash
# pde - Personal Dev Environment
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export SCRIPT_DIR

# Ensure cargo is in PATH for partial re-runs
export PATH="$HOME/.cargo/bin:$PATH"

source "$SCRIPT_DIR/lib/core.sh"

# Clean up temp directories on exit
cleanup() {
    rm -rf /tmp/tmux-build-$$ /tmp/yazi-$$ /tmp/font-*-$$ 2>/dev/null || true
}
trap cleanup EXIT

usage() {
    cat <<EOF
Usage: pde <profile>

Profiles:
  minimal   Full shell experience (any remote machine)
  full      Everything + AI tools + fonts (GUI machines)

Options:
  --help    Show this help
EOF
}

main() {
    local profile="${1:-}"

    case "$profile" in
        minimal)
            section "Installing minimal profile"
            need_sudo

            source "$SCRIPT_DIR/lib/shell.sh"
            source "$SCRIPT_DIR/lib/tools.sh"
            source "$SCRIPT_DIR/lib/editor.sh"

            install_shell      # zsh, tmux from source, plugins
            install_tools      # apt + cargo tools, yazi
            install_editor     # neovim + lazyvim
            install_configs    # all configs except ai
            ;;

        full)
            section "Installing full profile"
            need_sudo

            source "$SCRIPT_DIR/lib/shell.sh"
            source "$SCRIPT_DIR/lib/tools.sh"
            source "$SCRIPT_DIR/lib/editor.sh"
            source "$SCRIPT_DIR/lib/ai.sh"
            source "$SCRIPT_DIR/lib/fonts.sh"

            install_shell
            install_tools
            install_tools_full   # adds trash-cli
            install_editor
            install_ai           # nvm, node, claude
            install_fonts
            install_configs
            install_configs_ai   # adds ai() function, alacritty, aider
            ;;

        --help|-h)
            usage
            exit 0
            ;;

        *)
            usage
            exit 1
            ;;
    esac

    section "Done!"
}

main "$@"
```

## Lib Files

### lib/core.sh
```bash
#!/usr/bin/env bash
# Core helpers - sourced by all other libs

# Logging
section() { echo -e "\n==> $*"; }
log()     { echo "    $*"; }
warn()    { echo "WARN: $*" >&2; }
die()     { echo "ERROR: $*" >&2; exit 1; }

# Idempotency
has()       { command -v "$1" &>/dev/null; }
installed() { dpkg -l "$1" 2>/dev/null | grep -q "^ii"; }

# Sudo handling with keep-alive for long operations
need_sudo() {
    log "Requesting sudo (may prompt for password)..."
    sudo -v || die "sudo required"
    # Keep sudo alive in background for long operations (tmux build, etc.)
    (while true; do sudo -v; sleep 50; done) &
    SUDO_KEEPALIVE_PID=$!
    trap "kill $SUDO_KEEPALIVE_PID 2>/dev/null" EXIT
}

# Package installation
install_apt() {
    local to_install=()
    for pkg in "$@"; do
        installed "$pkg" || to_install+=("$pkg")
    done
    [[ ${#to_install[@]} -eq 0 ]] && return 0
    log "Installing: ${to_install[*]}"
    sudo apt-get update -qq
    sudo apt-get install -y "${to_install[@]}" || die "apt-get install failed"
}

install_cargo() {
    has cargo-binstall || die "cargo-binstall not found - run install_rust first"
    for tool in "$@"; do
        if ! has "$tool"; then
            log "Installing $tool via cargo-binstall..."
            "$HOME/.cargo/bin/cargo-binstall" -y --no-confirm "$tool" \
                || die "cargo-binstall $tool failed"
        fi
    done
}

# Config management - validates symlink points to correct source
link_config() {
    local src="$1" dest="$2"

    # Check if already correctly linked
    if [[ -L "$dest" ]]; then
        if [[ "$(readlink "$dest")" == "$src" ]]; then
            log "Already linked: $dest"
            return 0
        else
            log "Updating symlink: $dest (was pointing to $(readlink "$dest"))"
            rm "$dest"
        fi
    fi

    # Backup existing file
    if [[ -e "$dest" ]]; then
        log "Backing up: $dest"
        mv "$dest" "$dest.bak.$(date +%s)"
    fi

    # Create parent directory if needed
    mkdir -p "$(dirname "$dest")"

    log "Linking: $dest -> $src"
    ln -s "$src" "$dest"
}

clone_or_pull() {
    local repo="$1" dest="$2"
    export GIT_TERMINAL_PROMPT=0  # Fail fast instead of prompting for creds

    if [[ -d "$dest" ]]; then
        log "Updating: $dest"
        git -C "$dest" pull --quiet || warn "git pull failed for $dest (continuing)"
    else
        log "Cloning: $repo"
        git clone --depth 1 --quiet "$repo" "$dest" \
            || die "git clone failed for $repo"
    fi
}

# Download with error checking
download() {
    local url="$1" dest="$2"
    log "Downloading: $url"
    curl -fsSL "$url" -o "$dest" || die "Download failed: $url"
}

# PDE config directory
create_pde_config() {
    mkdir -p "$HOME/.config/pde"
    cat > "$HOME/.config/pde/paths.env" <<EOF
# Personal Dev Environment configuration
# Auto-generated by pde installer
export PDE_INSTALL_PATH="$SCRIPT_DIR"
export PDE_PROFILE="${PDE_PROFILE:-unknown}"
EOF
    log "Created ~/.config/pde/paths.env"
}
```

### lib/shell.sh
```bash
#!/usr/bin/env bash
# Shell setup: zsh, tmux (from source), plugins

TMUX_VERSION="3.6a"

install_shell() {
    section "Shell Environment"

    # System packages
    install_apt zsh git curl

    # Build tmux from source (need 3.2+ for modern features)
    install_tmux_from_source

    # Rust toolchain (needed for cargo-binstall tools)
    install_rust

    # Zsh plugin manager
    clone_or_pull "https://github.com/mattmc3/antidote.git" \
                  "$HOME/.local/share/antidote"

    # Powerlevel10k theme
    clone_or_pull "https://github.com/romkatv/powerlevel10k.git" \
                  "$HOME/.local/share/powerlevel10k"

    # Tmux plugin manager
    clone_or_pull "https://github.com/tmux-plugins/tpm" \
                  "$HOME/.tmux/plugins/tpm"

    # Set shell to zsh
    if [[ "$SHELL" != */zsh ]]; then
        log "Changing shell to zsh..."
        sudo chsh -s "$(which zsh)" "$USER"
    fi
}

install_tmux_from_source() {
    if has tmux && [[ "$(tmux -V)" == *"$TMUX_VERSION"* ]]; then
        log "tmux $TMUX_VERSION already installed"
        return 0
    fi

    section "Building tmux $TMUX_VERSION from source"
    install_apt build-essential libevent-dev libncurses-dev bison

    local tmp="/tmp/tmux-build-$$"
    local tarball="$tmp/tmux.tar.gz"
    mkdir -p "$tmp"

    download "https://github.com/tmux/tmux/releases/download/$TMUX_VERSION/tmux-$TMUX_VERSION.tar.gz" \
             "$tarball"

    tar xzf "$tarball" -C "$tmp" || die "Failed to extract tmux source"
    cd "$tmp/tmux-$TMUX_VERSION"

    ./configure --prefix=/usr/local || die "tmux configure failed"
    make -j"$(nproc)" || die "tmux build failed"
    sudo make install || die "tmux install failed"

    cd /
    rm -rf "$tmp"

    # Verify installation
    /usr/local/bin/tmux -V || die "tmux installation verification failed"
    log "tmux $TMUX_VERSION installed"
}

install_rust() {
    if has rustc; then
        log "Rust already installed"
    else
        section "Installing Rust"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
            || die "Rust installation failed"
    fi

    # Ensure cargo is in PATH for this session
    export PATH="$HOME/.cargo/bin:$PATH"

    # Verify rust works
    "$HOME/.cargo/bin/rustc" --version &>/dev/null \
        || die "Rust installation verification failed"

    if ! has cargo-binstall; then
        log "Installing cargo-binstall..."
        curl -L --proto '=https' --tlsv1.2 -sSf \
            https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh \
            | bash || die "cargo-binstall installation failed"
    fi
}
```

### lib/tools.sh
```bash
#!/usr/bin/env bash
# CLI tools: apt packages + cargo-binstall tools

install_tools() {
    section "CLI Tools"

    # Apt packages (available on all Ubuntu versions)
    # Note: unzip needed for yazi installation
    install_apt fd-find fzf ripgrep bat jq htop unzip

    # Cargo tools (not reliably available via apt)
    install_cargo eza zoxide

    # Yazi file manager
    install_yazi
}

install_tools_full() {
    section "CLI Tools (full additions)"
    install_apt trash-cli
}

install_yazi() {
    if has yazi; then
        log "yazi already installed"
        return 0
    fi

    section "Installing yazi"
    local arch
    case "$(uname -m)" in
        x86_64)  arch="x86_64-unknown-linux-gnu" ;;
        aarch64) arch="aarch64-unknown-linux-gnu" ;;
        *)       die "Unsupported architecture for yazi: $(uname -m)" ;;
    esac

    local tmp="/tmp/yazi-$$"
    mkdir -p "$tmp"

    download "https://github.com/sxyazi/yazi/releases/latest/download/yazi-$arch.zip" \
             "$tmp/yazi.zip"

    unzip -q "$tmp/yazi.zip" -d "$tmp" || die "Failed to extract yazi"

    mkdir -p "$HOME/.cargo/bin"
    cp "$tmp/yazi-$arch/yazi" "$tmp/yazi-$arch/ya" "$HOME/.cargo/bin/"
    chmod +x "$HOME/.cargo/bin/yazi" "$HOME/.cargo/bin/ya"

    rm -rf "$tmp"

    # Verify
    "$HOME/.cargo/bin/yazi" --version &>/dev/null || die "yazi installation verification failed"
    log "yazi installed"
}
```

### lib/editor.sh
```bash
#!/usr/bin/env bash
# Editor setup: neovim + lazyvim

NEOVIM_VERSION="stable"

install_editor() {
    section "Editor (Neovim + LazyVim)"

    install_neovim
    install_lazyvim
}

install_neovim() {
    if [[ -f /opt/nvim ]] && /opt/nvim --version &>/dev/null; then
        log "Neovim already installed"
        return 0
    fi

    # Architecture detection
    local arch
    case "$(uname -m)" in
        x86_64)  arch="x86_64" ;;
        aarch64) arch="aarch64" ;;
        *)       die "Unsupported architecture for neovim: $(uname -m)" ;;
    esac

    log "Installing Neovim ($arch)..."
    local url="https://github.com/neovim/neovim/releases/download/$NEOVIM_VERSION/nvim-linux-${arch}.appimage"

    sudo curl -fsSL "$url" -o /opt/nvim || die "Neovim download failed"
    sudo chmod +x /opt/nvim

    # Symlinks
    sudo ln -sf /opt/nvim /usr/local/bin/nvim
    sudo ln -sf /opt/nvim /usr/local/bin/vim

    # Verify
    /opt/nvim --version &>/dev/null || die "Neovim installation verification failed"
    log "Neovim installed"
}

install_lazyvim() {
    local nvim_config="$HOME/.config/nvim"

    if [[ -f "$nvim_config/lazy-lock.json" ]]; then
        log "LazyVim already installed"
        return 0
    fi

    log "Installing LazyVim starter..."

    # Backup existing config (preserves any custom config)
    if [[ -d "$nvim_config" ]]; then
        local backup="$nvim_config.bak.$(date +%s)"
        log "Backing up existing nvim config to $backup"
        mv "$nvim_config" "$backup"
    fi

    git clone --depth 1 https://github.com/LazyVim/starter "$nvim_config" \
        || die "LazyVim clone failed"
    rm -rf "$nvim_config/.git"

    log "LazyVim installed (previous config backed up if it existed)"
}
```

### lib/ai.sh
```bash
#!/usr/bin/env bash
# AI tools: nvm, node, claude

NVM_VERSION="v0.40.0"
NODE_VERSION="20"

install_ai() {
    section "AI Tools"

    install_nvm
    install_node
    install_claude
}

install_nvm() {
    if [[ -d "$HOME/.nvm" ]] && [[ -s "$HOME/.nvm/nvm.sh" ]]; then
        log "nvm already installed"
        return 0
    fi

    log "Installing nvm..."
    curl -o- "https://raw.githubusercontent.com/nvm-sh/nvm/$NVM_VERSION/install.sh" | bash \
        || die "nvm installation failed"

    # Verify nvm.sh exists after installation
    [[ -s "$HOME/.nvm/nvm.sh" ]] || die "nvm.sh not found after installation"
}

install_node() {
    export NVM_DIR="$HOME/.nvm"

    # Source nvm with verification
    if [[ -s "$NVM_DIR/nvm.sh" ]]; then
        # shellcheck source=/dev/null
        source "$NVM_DIR/nvm.sh"
    else
        die "nvm.sh not found - nvm installation may have failed"
    fi

    # Verify nvm is available
    type nvm &>/dev/null || die "nvm command not available after sourcing"

    if nvm ls "$NODE_VERSION" &>/dev/null; then
        log "Node $NODE_VERSION already installed"
        return 0
    fi

    log "Installing Node $NODE_VERSION..."
    nvm install "$NODE_VERSION" || die "Node installation failed"
    nvm alias default "$NODE_VERSION"
}

install_claude() {
    export NVM_DIR="$HOME/.nvm"
    # shellcheck source=/dev/null
    [[ -s "$NVM_DIR/nvm.sh" ]] && source "$NVM_DIR/nvm.sh"

    if npm list -g @anthropic-ai/claude-code &>/dev/null; then
        log "Claude Code already installed"
        return 0
    fi

    log "Installing Claude Code..."
    npm install -g @anthropic-ai/claude-code || die "Claude Code installation failed"

    # Verify
    command -v claude &>/dev/null || warn "claude command not in PATH (may need shell restart)"
}
```

### lib/fonts.sh
```bash
#!/usr/bin/env bash
# Nerd Fonts installation

NERD_FONTS_VERSION="v3.2.1"
FONTS_DIR="$HOME/.local/share/fonts"

install_fonts() {
    section "Nerd Fonts"

    # fontconfig provides fc-cache
    install_apt fontconfig

    mkdir -p "$FONTS_DIR"

    install_font "FiraCode" "zip"
    install_font "JetBrainsMono" "tar.xz"

    log "Refreshing font cache..."
    fc-cache -fv &>/dev/null || warn "fc-cache failed (fonts may still work)"
}

install_font() {
    local name="$1" ext="$2"
    local marker="$FONTS_DIR/.$name.installed"

    if [[ -f "$marker" ]]; then
        log "$name already installed"
        return 0
    fi

    log "Installing $name..."
    local url="https://github.com/ryanoasis/nerd-fonts/releases/download/$NERD_FONTS_VERSION/$name.$ext"
    local tmp="/tmp/font-$name-$$"

    mkdir -p "$tmp"
    download "$url" "$tmp/$name.$ext"

    case "$ext" in
        zip)    unzip -q "$tmp/$name.$ext" -d "$FONTS_DIR" || die "Failed to extract $name" ;;
        tar.xz) tar -xJf "$tmp/$name.$ext" -C "$FONTS_DIR" || die "Failed to extract $name" ;;
    esac

    touch "$marker"
    rm -rf "$tmp"
}
```

## Config Strategy

**Same zshrc everywhere** with conditional features:

```
config/zsh/zshrc       # Full config including tw(), conditional rm alias, sources zshrc.ai if exists
config/zsh/zshrc.ai    # ai() function only
```

### Key zshrc changes from current:

1. **Conditional rm alias** - Only use trash if installed:
```bash
# In zshrc - conditional trash alias (trash-cli only in full profile)
command -v trash &>/dev/null && alias rm='trash'
```

2. **Source AI config if present**:
```bash
# AI tools (installed separately by full profile)
[[ -f ~/.zshrc.ai ]] && source ~/.zshrc.ai
```

3. **tw() stays in main zshrc** - It's tmux-related, not AI-related

- `pde minimal` - Links zshrc only (rm works normally)
- `pde full` - Links zshrc AND zshrc.ai (rm uses trash)

## Config Installation

```bash
install_configs() {
    section "Config Files"

    # Create PDE config first (sets PDE_INSTALL_PATH for ai() function)
    PDE_PROFILE="minimal"
    create_pde_config

    link_config "$SCRIPT_DIR/config/zsh/zshrc" "$HOME/.zshrc"
    link_config "$SCRIPT_DIR/config/zsh/zsh_plugins.txt" "$HOME/.zsh_plugins.txt"
    link_config "$SCRIPT_DIR/config/tmux/tmux.conf" "$HOME/.tmux.conf"
    link_config "$SCRIPT_DIR/config/p10k/p10k.zsh" "$HOME/.p10k.zsh"
}

install_configs_ai() {
    section "AI Config Files"

    # Update PDE config with full profile
    PDE_PROFILE="full"
    create_pde_config

    link_config "$SCRIPT_DIR/config/zsh/zshrc.ai" "$HOME/.zshrc.ai"

    # Alacritty
    mkdir -p "$HOME/.config/alacritty"
    link_config "$SCRIPT_DIR/config/alacritty/alacritty.toml" \
                "$HOME/.config/alacritty/alacritty.toml"
}
```

## Makefile Retention

Keep a minimal Makefile for the `make claude` target that ai() function uses:

```makefile
# Makefile - Minimal version for claude launcher
# Full PDE installation: ./pde minimal|full

PROFILE ?= local
MODEL ?= openai/glm4.5-air-reap
LAUNCH_DIR ?= $(shell pwd)

.PHONY: claude
claude:
	cd $(LAUNCH_DIR) && claude --profile $(PROFILE) --model $(MODEL)
```

The ai() function in zshrc.ai will use `$PDE_INSTALL_PATH` (set in paths.env) to find this Makefile.

## Remote Installation

### bootstrap.sh
```bash
#!/usr/bin/env bash
# Bootstrap PDE installation from curl
set -euo pipefail

REPO_URL="https://git.blathers.goog/hco/pde.git"
INSTALL_DIR="$HOME/.pde"

main() {
    local profile="${1:-}"

    if [[ -z "$profile" ]]; then
        echo "Usage: curl ... | bash -s -- <minimal|full>"
        exit 1
    fi

    # Install git if missing
    if ! command -v git &>/dev/null; then
        echo "Installing git..."
        sudo apt-get update -qq
        sudo apt-get install -y git
    fi

    # Clone or update
    if [[ -d "$INSTALL_DIR" ]]; then
        echo "Updating existing PDE installation..."
        git -C "$INSTALL_DIR" pull
    else
        echo "Cloning PDE..."
        git clone "$REPO_URL" "$INSTALL_DIR"
    fi

    # Run installer
    "$INSTALL_DIR/pde" "$profile"
}

main "$@"
```

Usage:
```bash
# Bootstrap from curl
curl -fsSL https://git.blathers.goog/hco/pde/raw/branch/main/bootstrap.sh | bash -s -- minimal

# Or clone and run directly
git clone ssh://git@git.blathers.goog:3022/hco/pde.git ~/.pde
~/.pde/pde full
```

## Migration Checklist

After implementation is tested:

**Remove:**
- [ ] site.yml, system.yml, user.yml, main.yml
- [ ] defaults.yml, inventory.yml, hosts.ini, ansible.cfg
- [ ] group_vars/
- [ ] vars/
- [ ] tasks/
- [ ] Old Makefile targets (keep only `claude` target)

**Keep:**
- [ ] config/ (reorganized)
- [ ] docs/
- [ ] README.md (rewritten)
- [ ] Makefile (minimal, for `make claude` only)

**Clean up orphaned files:**
- [ ] config/screen/.screenrc - Remove (screen not supported)
- [ ] config/xfce4/ - Remove entire directory (not deployed)
- [ ] config/nvim/ - Remove after verifying LazyVim works (old custom config)
- [ ] config/aider/ - Remove entire directory (aider not used)

## Implementation Order

1. [ ] lib/core.sh - Helpers with error handling, sudo keepalive
2. [ ] lib/shell.sh - zsh, tmux from source, rust, plugins
3. [ ] lib/tools.sh - apt + cargo tools, yazi (with unzip dep)
4. [ ] lib/editor.sh - neovim (arch-aware), lazyvim
5. [ ] lib/ai.sh - nvm, node, claude (with verification)
6. [ ] lib/fonts.sh - nerd fonts (with fontconfig dep)
7. [ ] Rename config files:
   - config/zsh/.zshrc -> config/zsh/zshrc
   - config/zsh/.zsh_plugins.txt -> config/zsh/zsh_plugins.txt
   - config/tmux/.tmux.conf -> config/tmux/tmux.conf
   - config/powerlevel10k/.p10k.zsh -> config/p10k/p10k.zsh
8. [ ] Update config/zsh/zshrc:
   - Make rm=trash alias conditional
   - Add `[[ -f ~/.zshrc.ai ]] && source ~/.zshrc.ai` at end
9. [ ] Create config/zsh/zshrc.ai (extract ai() function)
10. [ ] Main pde script
11. [ ] bootstrap.sh
12. [ ] Minimal Makefile (claude target only)
13. [ ] Test minimal on fresh Ubuntu 22.04 VM
14. [ ] Test minimal on fresh Ubuntu 24.04 VM
15. [ ] Test full on fresh Ubuntu 24.04 VM
16. [ ] Test re-run (idempotency) on both profiles
17. [ ] Remove Ansible files
18. [ ] Remove orphaned config files
19. [ ] Update README.md
20. [ ] Update .gitignore

## Testing Checklist

For each test VM:

**Prerequisites:**
- [ ] Fresh Ubuntu install (no prior PDE)
- [ ] User has sudo access
- [ ] Network connectivity

**After `pde minimal`:**
- [ ] zsh is default shell
- [ ] tmux 3.6a installed (`tmux -V`)
- [ ] p10k prompt displays correctly
- [ ] eza, zoxide, bat, fzf, fd, rg, jq work
- [ ] yazi launches
- [ ] neovim launches, LazyVim loads
- [ ] tw() function works in tmux
- [ ] rm command works (normal rm, not trash)

**After `pde full` (in addition to above):**
- [ ] trash-cli works (`rm` uses trash)
- [ ] claude command available
- [ ] ai() function works
- [ ] Nerd Fonts installed

**Idempotency:**
- [ ] Running same profile twice completes without errors
- [ ] No duplicate backups created
- [ ] Symlinks unchanged on re-run
