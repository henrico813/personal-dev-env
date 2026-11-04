#!/bin/bash
# Changes the backlight by a percentage and shows the new level.
# Usage: brightness-notify.sh 5%+
#        brightness-notify.sh 5%-
# logind sets the level, so the user needs no video group.

if [[ ! "${1:-}" =~ ^([0-9]+)%([+-])$ ]]; then
    echo "usage: brightness-notify.sh <percent>%+|<percent>%-" >&2
    exit 2
fi
percent=${BASH_REMATCH[1]}
direction=${BASH_REMATCH[2]}

device=""
for path in /sys/class/backlight/*; do
    if [[ -r "$path/brightness" ]]; then
        device=${path##*/}
        break
    fi
done
if [[ -z "$device" ]]; then
    exit 0  # No backlight, such as a desktop or a VM
fi

current=$(<"/sys/class/backlight/$device/brightness")
max=$(<"/sys/class/backlight/$device/max_brightness")
step=$(( max * percent / 100 ))
if [[ "$direction" == "+" ]]; then
    value=$(( current + step > max ? max : current + step ))
else
    value=$(( current - step < 0 ? 0 : current - step ))
fi

busctl call org.freedesktop.login1 \
    /org/freedesktop/login1/session/auto \
    org.freedesktop.login1.Session SetBrightness \
    ssu backlight "$device" "$value" || exit 1
level=$(( value * 100 / max ))
dunstify -a "brightness" -u low -r 9999 -h int:value:"$level" "Brightness: ${level}%"
