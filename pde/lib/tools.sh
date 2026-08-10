#!/usr/bin/env bash
# CLI tools: apt packages + cargo-binstall tools

install_tools() {
    section "CLI Tools"

    # Apt packages (available on all Ubuntu versions)
    # Note: unzip needed for yazi installation
    install_apt fd-find ripgrep bat jq htop unzip keychain

    # Define specific versions of tools
    FZF_VERSION="0.36.0" # need v0.36.0 for nvim

    # Install those specific versions of tools
    install_fzf

    # Cargo tools (not reliably available via apt)
    install_cargo eza zoxide

    # Binary downloads (not available via apt or cargo)
    install_btm
    install_yq
    install_yazi
}

install_tools_full() {
    section "CLI Tools (full additions)"
    install_apt trash-cli
}

install_fzf() {
  local binary="/usr/local/bin/fzf"

  if [[ -x "$binary" ]] && [[ "$("$binary" --version)" == "$FZF_VERSION"* ]]; then
    log "fzf $FZF_VERSION already installed"
    return 0
  fi
  log "fzf "$(fzf --version)" detected, installing fzf $FZF_VERSION"

  local arch
  case "$(uname -m)" in
    x86_64)   arch="amd64" ;;
    aarch64)  arch="arm64" ;;
    * ) die "Unsupported architecture for fzf: $(uname -m)" ;;
  esac

  local tmp="/tmp/fzf-$$"
  mkdir -p "$tmp"

  download \
    "https://github.com/junegunn/fzf/releases/download/$FZF_VERSION/fzf-$FZF_VERSION-linux_$arch.tar.gz" \
    "$tmp/fzf.tar.gz"
  tar -xzf "$tmp/fzf.tar.gz" -C "$tmp" || die "Failed to extract fzf"

  need_sudo
  sudo install -m 0755 "$tmp/fzf" "$binary"
  rm -rf "$tmp"

  [[ "$("$binary" --version)" == "$FZF_VERSION"* ]] ||
    die "fzf $FZF_VERSION installation verification failed"
}

install_btm() {
    if has btm; then
        log "btm already installed"
        return 0
    fi

    has cargo-binstall || die "cargo-binstall not found - run install_rust first"

    log "Installing btm..."
    "$HOME/.cargo/bin/cargo-binstall" -y bottom \
        || die "cargo-binstall bottom failed"

    [[ -x "$HOME/.cargo/bin/btm" ]] || die "btm installation verification failed"
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
