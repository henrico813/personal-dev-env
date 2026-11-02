#!/bin/bash
# Rofi power menu for i3 and Sway (requires Nerd Font)

if [[ -n "${SWAYSOCK:-}" ]]; then
    lock=(swaylock -f -c 1a1b26)
    logout=(swaymsg exit)
else
    lock=(i3lock -c 282a36)
    logout=(i3-msg exit)
fi

options="  Lock\n  Logout\n  Suspend\n  Reboot\n  Shutdown"

selected=$(echo -e "$options" | rofi -dmenu -i -p "󰐥" \
    -theme-str 'window {width: 250px;} listview {lines: 5;}')

confirm() {
    echo -e "Yes\nNo" | rofi -dmenu -i -p "$1?" -theme-str 'window {width: 150px;}'
}

case "$selected" in
    *Lock) "${lock[@]}" ;;
    *Logout) [[ $(confirm "Logout") == "Yes" ]] && "${logout[@]}" ;;
    *Suspend) systemctl suspend ;;
    *Reboot) [[ $(confirm "Reboot") == "Yes" ]] && systemctl reboot ;;
    *Shutdown) [[ $(confirm "Shutdown") == "Yes" ]] && systemctl poweroff ;;
esac
