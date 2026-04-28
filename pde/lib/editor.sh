#!/usr/bin/env bash
# Editor setup: Neovim + PDE nvim config

NEOVIM_VERSION="stable"

install_editor() {
    local config_dir="$HOME/.config/nvim"
    local src_dir="$SCRIPT_DIR/config/nvim"

    section "Editor (Neovim + PDE nvim)"

    install_neovim

    # Preserve any preexisting config before the PDE tree takes over.
    if [[ -e "$config_dir" || -L "$config_dir" ]]; then
        if [[ -L "$config_dir" && "$(readlink "$config_dir")" == "$src_dir" ]]; then
            log "PDE nvim config already linked"
        else
            local backup="$config_dir.bak.$(date +%s)"
            log "Backing up existing nvim config to $backup"
            mv "$config_dir" "$backup"
        fi
    fi

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
        "https://github.com/sindrets/diffview.nvim"
        "https://github.com/mason-org/mason.nvim"
        "https://github.com/mason-org/mason-lspconfig.nvim"
        "https://github.com/neovim/nvim-lspconfig"
        "https://github.com/MeanderingProgrammer/render-markdown.nvim"
        "https://github.com/mfussenegger/nvim-lint"
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

    # Install pi-nvim wrapper (launches pi without the buggy pi-pretty extension)
    local user_bin="$HOME/.local/bin"
    mkdir -p "$user_bin"
    link_config "$src_dir/bin/pi-nvim" "$user_bin/pi-nvim"
    chmod +x "$src_dir/bin/pi-nvim"

    # Build blink.cmp native fuzzy library.
    if command -v cargo &>/dev/null && [[ -d "$pack_dir/blink.cmp" ]]; then
        log "Building blink.cmp..."
        (cd "$pack_dir/blink.cmp" && cargo build --release --quiet)

        local commit_hash
        commit_hash=$(git -C "$pack_dir/blink.cmp" rev-parse HEAD 2>/dev/null | cut -c1-7)
        local lib_dir="$HOME/.local/share/nvim/site/lib"
        mkdir -p "$lib_dir"
        ln -sf "$pack_dir/blink.cmp/target/release/libblink_cmp_fuzzy.so" \
               "$lib_dir/libblink_cmp_fuzzy.so.$commit_hash"
        log "blink.cmp built"
    else
        log "Skipping blink.cmp build (cargo not found)"
    fi
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
