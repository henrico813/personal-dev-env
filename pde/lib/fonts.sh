#!/usr/bin/env bash
# Nerd Fonts installation

NERD_FONTS_VERSION="v3.2.1"
FONTS_DIR="$HOME/.local/share/fonts"

install_fonts() {
    section "Nerd Fonts"

    # fontconfig provides fc-cache
    install_apt fontconfig

    mkdir -p "$FONTS_DIR"

    install_font "FiraCode" "zip"
    install_font "JetBrainsMono" "tar.xz"

    log "Refreshing font cache..."
    fc-cache -fv &>/dev/null || warn "fc-cache failed (fonts may still work)"
}

install_font() {
    local name="$1" ext="$2"
    local marker="$FONTS_DIR/.$name.installed"

    if [[ -f "$marker" ]]; then
        log "$name already installed"
        return 0
    fi

    log "Installing $name..."
    local url="https://github.com/ryanoasis/nerd-fonts/releases/download/$NERD_FONTS_VERSION/$name.$ext"
    local tmp="/tmp/font-$name-$$"

    mkdir -p "$tmp"
    download "$url" "$tmp/$name.$ext"

    case "$ext" in
        zip)    unzip -q "$tmp/$name.$ext" -d "$FONTS_DIR" || die "Failed to extract $name" ;;
        tar.xz) tar -xJf "$tmp/$name.$ext" -C "$FONTS_DIR" || die "Failed to extract $name" ;;
    esac

    touch "$marker"
    rm -rf "$tmp"
}
