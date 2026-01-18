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
        sudo chsh -s "$(which zsh)" "$(whoami)"
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
