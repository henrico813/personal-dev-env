#!/usr/bin/env bash
# CLI tools: apt packages + Aqua-managed tools

AQUA_VERSION="v2.60.1"
AQUA_INSTALLER_VERSION="v4.0.5"
AQUA_ROOT_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/aquaproj-aqua"
AQUA_BIN="$AQUA_ROOT_DIR/bin/aqua"

install_tools() {
    section "CLI Tools"

    # OS-integrated tools and prerequisites.
    install_apt htop unzip keychain xclip

    install_aqua
    install_aqua_tools
}

install_tools_full() {
    section "CLI Tools (full additions)"
    install_apt trash-cli
}

install_aqua() {
    if [[ -x "$AQUA_BIN" ]] && [[ "$("$AQUA_BIN" --version)" == "aqua version ${AQUA_VERSION#v}"* ]]; then
        log "Aqua $AQUA_VERSION already installed"
        return 0
    fi

    section "Installing Aqua $AQUA_VERSION"
    curl -sSfL "https://raw.githubusercontent.com/aquaproj/aqua-installer/$AQUA_INSTALLER_VERSION/aqua-installer" |
        AQUA_ROOT_DIR="$AQUA_ROOT_DIR" bash -s -- -v "$AQUA_VERSION" || die "Aqua installation failed"

    [[ -x "$AQUA_BIN" ]] || die "Aqua installation verification failed"
}

install_aqua_tools() {
    export AQUA_ROOT_DIR
    export AQUA_GLOBAL_CONFIG="$SCRIPT_DIR/config/home/.config/aquaproj-aqua/aqua.yaml"
    export PATH="$AQUA_ROOT_DIR/bin:$PATH"

    "$AQUA_BIN" --config "$AQUA_GLOBAL_CONFIG" install || die "Aqua tool installation failed"

    local tool
    for tool in fd fzf rg bat jq chezmoi eza zoxide btm yq yazi ya; do
        [[ -x "$AQUA_ROOT_DIR/bin/$tool" ]] || die "$tool installation verification failed"
    done

    # Aqua's musl Yazi replaces the legacy Cargo binary incompatible with Jammy.
    rm -f "$HOME/.cargo/bin/yazi" "$HOME/.cargo/bin/ya"
}
