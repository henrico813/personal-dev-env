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
