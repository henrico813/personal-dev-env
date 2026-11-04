#!/bin/bash
# Opens the Super+q menu, or one desktop tool by name.
# Example: desktop-menu.sh bluetooth, or desktop-menu.sh menu -filter blue
# Inside rofi it is the "desktop" script mode, which lists the four tools.
set -uo pipefail

self=$(readlink -f "$0")
here=$(dirname "$self")
dim='#859289'
accent='#7fbbb3'
fg='#d3c6aa'

# GNOME Settings hides its Wi-Fi page outside a GNOME session, and starts
# light there because no GNOME portal reports the dark setting.
settings() {
    XDG_CURRENT_DESKTOP=GNOME ADW_DEBUG_COLOR_SCHEME=prefer-dark setsid -f gnome-control-center "$1" >/dev/null 2>&1
    # env -u keeps rofi's variables out of the helper's environment.
    env -u ROFI_RETV setsid -f "$here/show-when-drawn.sh" gnome-control-center >/dev/null 2>&1
}

open() {
    case "$1" in
        wifi) settings wifi ;;
        bluetooth) settings bluetooth ;;
        # Xfce's tool uses xrandr; GNOME's Displays page needs Mutter.
        displays)
            setsid -f xfce4-display-settings --class=pde-displays >/dev/null 2>&1
            env -u ROFI_RETV setsid -f "$here/show-when-drawn.sh" pde-displays >/dev/null 2>&1
            ;;
        # The power menu is a second rofi, which cannot start while Super+q's
        # rofi still holds the keyboard, so wait for that one to close.
        power)
            env -u ROFI_RETV -u ROFI_INFO setsid -f bash -c \
                'while pgrep -x rofi >/dev/null; do sleep 0.05; done; exec ~/.config/pde-desktop/scripts/powermenu.sh' \
                >/dev/null 2>&1
            ;;
    esac
}

# Help text spans, as rofi markup.
keys() {
    local line=""
    while (($# >= 2)); do
        [[ -n "$line" ]] && line+="<span foreground='$dim'> | </span>"
        line+="<span foreground='$accent'>$1</span> <span foreground='$fg'>$2</span>"
        shift 2
    done
    printf '%s' "$line"
}

# rofi runs this script as a mode: ROFI_RETV 0 lists entries, 1 picks one.
if [[ -n "${ROFI_RETV:-}" ]]; then
    if [[ "$ROFI_RETV" == 1 ]]; then
        open "${ROFI_INFO:-}"
        exit 0
    fi
    printf '\0markup-rows\x1ftrue\n'
    entry() {
        printf "%s  <span foreground='%s'>%s</span>\0info\x1f%s\x1ficon\x1f%s\x1fmeta\x1f%s\n" \
            "$1" "$dim" "$2" "$3" "$4" "$5"
    }
    entry Wi-Fi Super+n wifi network-wireless "wifi wireless network"
    entry Bluetooth Super+b bluetooth bluetooth "bluetooth"
    entry Displays Super+p displays video-display "displays monitors screens"
    entry Power Super+x power system-shutdown "power lock logout reboot shutdown"
    exit 0
fi

case "${1:-menu}" in
    menu)
        help=$(keys C-j/C-k Move C-h/C-l Mode Enter Open Esc Close)
        exec rofi -show combi -modi "combi,window" -show-icons \
            -combi-modi "desktop:$self,drun" -combi-display-format '{text}' -display-combi Apps \
            -theme-str "window {width: 880px;} mainbox {children: [inputbar, listview, mode-switcher, textbox-help];}
                textbox-help {content: \"$help\";}" \
            "${@:2}"
        ;;
    *) open "$1" ;;
esac
