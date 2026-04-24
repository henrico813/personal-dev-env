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

install_nvim_minimal() {
    local config_dir="$HOME/.config/nvim-minimal"
    local src_dir="$SCRIPT_DIR/config/nvim-minimal"

    section "Editor (nvim-minimal)"

    link_config "$src_dir" "$config_dir"

    local pack_dir="$config_dir/pack/plugins/start"
    mkdir -p "$pack_dir"

    local plugins=(
        "https://github.com/folke/tokyonight.nvim"
        "https://github.com/nvim-lualine/lualine.nvim"
        "https://github.com/akinsho/bufferline.nvim"
        "https://github.com/ibhagwan/fzf-lua"
        "https://github.com/Saghen/blink.cmp"
        "https://github.com/Saghen/blink.lib"
        "https://github.com/folke/which-key.nvim"
        "https://github.com/lewis6991/gitsigns.nvim"
        "https://github.com/folke/trouble.nvim"
        "https://github.com/MagicDuck/grug-far.nvim"
        "https://github.com/linrongbin16/gitlinker.nvim"
        "https://github.com/folke/persistence.nvim"
        "https://github.com/goolord/alpha-nvim"
        "https://github.com/alex35mil/pi.nvim"
    )

    for url in "${plugins[@]}"; do
        local name="${url##*/}"
        if [[ ! -d "$pack_dir/$name" ]]; then
            log "Cloning $name..."
            git clone --depth=1 "$url" "$pack_dir/$name"
        else
            log "$name already installed"
        fi
    done

    # Build blink.cmp native fuzzy library
    if command -v cargo &>/dev/null && [[ -d "$pack_dir/blink.cmp" ]]; then
        log "Building blink.cmp..."
        (cd "$pack_dir/blink.cmp" && cargo build --release --quiet)

        local commit_hash
        commit_hash=$(git -C "$pack_dir/blink.cmp" rev-parse HEAD 2>/dev/null | cut -c1-7)
        local lib_dir="$HOME/.local/share/nvim-minimal/site/lib"
        mkdir -p "$lib_dir"
        ln -sf "$pack_dir/blink.cmp/target/release/libblink_cmp_fuzzy.so" \
               "$lib_dir/libblink_cmp_fuzzy.so.$commit_hash"
        log "blink.cmp built"
    else
        log "Skipping blink.cmp build (cargo not found)"
    fi
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
    local pde_nvim_plugins="$SCRIPT_DIR/config/nvim/lua/plugins"
    if [[ -d "$pde_nvim_plugins" ]]; then
        log "Syncing custom nvim plugins..."
        cp -r "$pde_nvim_plugins"/* "$nvim_config/lua/plugins/"
    fi
}
