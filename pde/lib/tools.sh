#!/usr/bin/env bash
# CLI tools: apt packages + cargo-binstall tools

install_tools() {
    section "CLI Tools"

    # Apt packages (available on all Ubuntu versions)
    # Note: unzip needed for yazi installation
    install_apt fd-find fzf ripgrep bat jq htop unzip keychain

    # Cargo tools (not reliably available via apt)
    install_cargo eza zoxide

    # Binary downloads (not available via apt or cargo)
    install_yq
    install_yazi
}

install_tools_full() {
    section "CLI Tools (full additions)"
    install_apt trash-cli
}

install_yq() {
    if has yq; then
        log "yq already installed"
        return 0
    fi

    section "Installing yq"
    local arch
    case "$(uname -m)" in
        x86_64)  arch="amd64" ;;
        aarch64) arch="arm64" ;;
        *)       die "Unsupported architecture for yq: $(uname -m)" ;;
    esac

    local tmp="/tmp/yq-$$"
    mkdir -p "$tmp"

    download "https://github.com/mikefarah/yq/releases/latest/download/yq_linux_$arch" \
             "$tmp/yq"

    mkdir -p "$HOME/.cargo/bin"
    cp "$tmp/yq" "$HOME/.cargo/bin/yq"
    chmod +x "$HOME/.cargo/bin/yq"

    rm -rf "$tmp"

    [[ -x "$HOME/.cargo/bin/yq" ]] || die "yq installation verification failed"
    log "yq installed"
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

    # Verify binary exists and is executable
    [[ -x "$HOME/.cargo/bin/yazi" ]] || die "yazi installation verification failed"
    log "yazi installed"
}
