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

check_version() {
    local cmd="$1"
    local expected="$2"
    local actual

    actual=$("$cmd" --version 2>&1) || fail "$cmd --version failed"
    actual="${actual%%$'\n'*}"
    [[ "$actual" == *"$expected"* ]] || fail "$cmd version is $actual, expected $expected"
    pass "$cmd version $expected"
}

check_absent() {
    local path="$1"

    [[ ! -e "$path" && ! -L "$path" ]] || fail "$path should be absent"
    pass "$path absent"
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

AQUA_ROOT_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/aquaproj-aqua"
AQUA_BIN="$AQUA_ROOT_DIR/bin"
export AQUA_GLOBAL_CONFIG="$HOME/.config/aquaproj-aqua/aqua.yaml"
export PATH="$AQUA_BIN:$PATH"

# Core tools (both profiles)
check_cmd zsh
check_cmd rustc ~/.cargo/bin/rustc
check_cmd aqua "$AQUA_BIN/aqua"
check_version "$AQUA_BIN/aqua" "aqua version 2.60.1"
check_version "$AQUA_BIN/fd" "fd 8.3.1"
check_version "$AQUA_BIN/fzf" "0.36.0"
check_version "$AQUA_BIN/rg" "ripgrep 14.1.1"
check_version "$AQUA_BIN/bat" "bat 0.19.0"
check_version "$AQUA_BIN/jq" "jq-1.7.1"
check_version "$AQUA_BIN/eza" "eza - A modern"
check_version "$AQUA_BIN/zoxide" "zoxide 0.9.8"
check_version "$AQUA_BIN/btm" "bottom 0.11.4"
check_version "$AQUA_BIN/yq" "v4.53.3"
check_version "$AQUA_BIN/yazi" "Yazi 25.5.31"
check_version "$AQUA_BIN/ya" "Ya 25.5.31"
check_absent ~/.cargo/bin/yazi
check_absent ~/.cargo/bin/ya

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
# PDE nvim config is linked into the default Neovim app dir
check_file ~/.config/nvim/init.lua "PDE nvim config"
check_dir ~/.config/nvim/pack/plugins/start "PDE nvim plugin pack"
check_link ~/.config/nvim "pde/config/nvim"
check_link ~/.config/aquaproj-aqua/aqua.yaml "pde/config/aqua/aqua.yaml"
check_link ~/.config/aquaproj-aqua/aqua-checksums.json "pde/config/aqua/aqua-checksums.json"

codecompanion_dir=~/.config/nvim/pack/plugins/start/codecompanion.nvim
check_dir "$codecompanion_dir" "CodeCompanion plugin"

codecompanion_origin=$(git -C "$codecompanion_dir" remote get-url origin 2>/dev/null || true)
if [[ "$codecompanion_origin" == "https://github.com/henrico813/codecompanion.nvim" ]]; then
    pass "CodeCompanion fork remote"
else
    fail "CodeCompanion remote is $codecompanion_origin, expected PDE fork"
fi

codecompanion_branch=$(git -C "$codecompanion_dir" branch --show-current 2>/dev/null || true)
if [[ "$codecompanion_branch" == "main" ]]; then
    pass "CodeCompanion fork branch main"
else
    fail "CodeCompanion branch is $codecompanion_branch, expected main"
fi

# PDE config is owned by the Go CLI; installer smoke tests stay runtime-focused.

# Config symlinks (match the config suffix so both standalone and combined repo installs pass)
check_link ~/.zshrc "pde/config/zsh/zshrc"
check_link ~/.tmux.conf "pde/config/tmux/tmux.conf"
check_link ~/.p10k.zsh "pde/config/p10k/p10k.zsh"
check_link ~/.config/bottom/bottom.toml "pde/config/bottom/bottom.toml"

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
