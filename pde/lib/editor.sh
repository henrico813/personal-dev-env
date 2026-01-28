#!/usr/bin/env bash
# Editor setup: neovim + lazyvim

NEOVIM_VERSION="stable"

install_editor() {
    section "Editor (Neovim + LazyVim)"

    install_neovim
    install_lazyvim
}

install_neovim() {
    if [[ -x /usr/local/bin/nvim ]] && /usr/local/bin/nvim --version &>/dev/null; then
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
    local tmp="/tmp/nvim-$$"

    mkdir -p "$tmp"
    curl -fsSL "$url" -o "$tmp/nvim.appimage" || die "Neovim download failed"
    chmod +x "$tmp/nvim.appimage"

    # Extract AppImage (works without FUSE)
    cd "$tmp"
    ./nvim.appimage --appimage-extract &>/dev/null || die "Failed to extract Neovim AppImage"

    # Install extracted version
    sudo rm -rf /opt/nvim
    sudo mv squashfs-root /opt/nvim
    sudo ln -sf /opt/nvim/usr/bin/nvim /usr/local/bin/nvim
    sudo ln -sf /opt/nvim/usr/bin/nvim /usr/local/bin/vim

    cd /
    rm -rf "$tmp"

    # Verify
    /usr/local/bin/nvim --version &>/dev/null || die "Neovim installation verification failed"
    log "Neovim installed"
}

install_lazyvim() {
    local nvim_config="$HOME/.config/nvim"

    if [[ -f "$nvim_config/lazy-lock.json" ]]; then
        log "LazyVim already installed"
    else
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
    fi

    # Always sync custom plugins from pde config
    local pde_nvim_plugins="$SCRIPT_DIR/../config/nvim/lua/plugins"
    if [[ -d "$pde_nvim_plugins" ]]; then
        log "Syncing custom nvim plugins..."
        cp -r "$pde_nvim_plugins"/* "$nvim_config/lua/plugins/"
    fi
}
