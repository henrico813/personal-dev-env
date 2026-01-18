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
