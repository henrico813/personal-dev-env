#!/usr/bin/env bash
# Verification script for PDE installation
# Run inside container after ./pde install
set -euo pipefail

PROFILE="${1:-minimal}"
EXPECTED_TMUX="${2:-3.6a}"

pass() { echo "PASS: $*"; }
fail() { echo "FAIL: $*"; exit 1; }

# Check command exists (by name or path)
check_cmd() {
    local cmd="$1"
    local path="${2:-}"
    if command -v "$cmd" &>/dev/null || [[ -n "$path" && -x "$path" ]]; then
        pass "$cmd"
    else
        fail "$cmd not found"
    fi
}

# Check file exists
check_file() {
    local path="$1"
    local desc="${2:-$path}"
    if [[ -f "$path" ]]; then
        pass "$desc"
    else
        fail "$desc not found"
    fi
}

# Check directory exists
check_dir() {
    local path="$1"
    local desc="${2:-$path}"
    if [[ -d "$path" ]]; then
        pass "$desc"
    else
        fail "$desc not found"
    fi
}

# Check symlink exists and points to expected target
check_link() {
    local path="$1"
    local expected_target="${2:-}"
    if [[ ! -L "$path" ]]; then
        fail "$path not symlinked"
    fi
    if [[ -n "$expected_target" ]]; then
        local actual
        actual=$(readlink "$path")
        if [[ "$actual" != *"$expected_target"* ]]; then
            fail "$path points to $actual, expected $expected_target"
        fi
    fi
    pass "$path symlinked"
}

echo "--- Verification ($PROFILE profile) ---"

# Core tools (both profiles)
check_cmd zsh
check_cmd rustc ~/.cargo/bin/rustc
check_cmd cargo-binstall ~/.cargo/bin/cargo-binstall
check_cmd eza ~/.cargo/bin/eza
check_cmd zoxide ~/.cargo/bin/zoxide
check_cmd fzf
check_cmd rg
check_cmd bat /usr/bin/batcat
check_cmd jq
check_cmd yazi ~/.cargo/bin/yazi
check_cmd ya ~/.cargo/bin/ya

# Tmux: check exists before checking version
if [[ ! -x /usr/local/bin/tmux ]]; then
    fail "tmux not found at /usr/local/bin/tmux"
fi
pass "tmux"

actual_tmux=$(/usr/local/bin/tmux -V)
if [[ "$actual_tmux" == *"$EXPECTED_TMUX"* ]]; then
    pass "tmux version $EXPECTED_TMUX"
else
    fail "tmux version is $actual_tmux, expected $EXPECTED_TMUX"
fi

# Neovim: check exists before checking version
if [[ ! -x /usr/local/bin/nvim ]]; then
    fail "nvim not found at /usr/local/bin/nvim"
fi
pass "nvim"

# Plugin directories
check_dir ~/.local/share/antidote "antidote plugin manager"
check_dir ~/.local/share/powerlevel10k "powerlevel10k theme"
check_dir ~/.tmux/plugins/tpm "tmux plugin manager"
# LazyVim starter has init.lua (lazy-lock.json only created on first run)
check_file ~/.config/nvim/init.lua "LazyVim config"

# PDE config
check_file ~/.config/pde/paths.env "PDE paths.env"

# Config symlinks (verify they point to .pde)
check_link ~/.zshrc ".pde/config/zsh/zshrc"
check_link ~/.tmux.conf ".pde/config/tmux/tmux.conf"
check_link ~/.p10k.zsh ".pde/config/p10k/p10k.zsh"

# Runtime verification - actually run the tools
echo "--- Runtime checks ---"

# Test zsh can start (plugins load)
if zsh -c 'echo ok' &>/dev/null; then
    pass "zsh starts"
else
    fail "zsh fails to start"
fi

# Test tmux can create session
if tmux new-session -d -s pde-test 2>/dev/null && tmux kill-session -t pde-test 2>/dev/null; then
    pass "tmux creates session"
else
    fail "tmux fails to create session"
fi

# Test nvim can start (--version doesn't trigger plugin downloads)
if /usr/local/bin/nvim --version &>/dev/null; then
    pass "nvim runs"
else
    fail "nvim fails to run"
fi

# Full profile extras
if [[ "$PROFILE" == "full" ]]; then
    echo "--- Full profile checks ---"
    check_cmd trash

    # Node: check nvm directory exists first
    if [[ ! -d ~/.nvm/versions/node ]]; then
        fail "nvm node versions directory not found"
    fi
    node_bin=$(find ~/.nvm/versions/node -name node -type f 2>/dev/null | head -1)
    if [[ -x "$node_bin" ]]; then
        pass "node"
    else
        fail "node not found"
    fi

    claude_bin=$(find ~/.nvm/versions/node -name claude \( -type f -o -type l \) 2>/dev/null | head -1)
    if [[ -x "$claude_bin" ]]; then
        pass "claude"
    else
        fail "claude not found"
    fi

    # Font marker files
    check_file ~/.local/share/fonts/.FiraCode.installed "FiraCode font"
    check_file ~/.local/share/fonts/.JetBrainsMono.installed "JetBrainsMono font"

    check_link ~/.config/alacritty/alacritty.toml ".pde/config/alacritty"
fi

echo "--- All checks passed ---"
