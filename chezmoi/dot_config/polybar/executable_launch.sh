#!/bin/bash

# Terminate already running bar instances
killall -q polybar

# Wait until the processes have been shut down
while pgrep -u $UID -x polybar >/dev/null; do sleep 1; done

CONFIG="$HOME/.config/polybar/config.ini"

# Auto-detect wireless interface
WLAN_IFACE=$(ip link show | grep -oE 'wl[a-z0-9]+' | head -1)

# Find the battery and charger, whose names differ between machines
BATTERY=""
ADAPTER=""
for supply in /sys/class/power_supply/*; do
    # Skip device batteries, such as a wireless mouse's, which can sort first.
    [[ "$(cat "$supply/scope" 2>/dev/null)" == "Device" ]] && continue
    case "$(cat "$supply/type" 2>/dev/null)" in
        Battery) BATTERY=${BATTERY:-${supply##*/}} ;;
        Mains) ADAPTER=${ADAPTER:-${supply##*/}} ;;
    esac
done

# Launch polybar on primary monitor only
PRIMARY=$(xrandr --query | grep " primary" | cut -d" " -f1)
# Fallback to first connected monitor if no primary set
if [[ -z "$PRIMARY" ]]; then
    PRIMARY=$(xrandr --query | grep " connected" | head -1 | cut -d" " -f1)
fi
MONITOR=${PRIMARY} WLAN_IFACE=${WLAN_IFACE:-wlan0} \
    BATTERY=${BATTERY} ADAPTER=${ADAPTER} \
    polybar --reload --config="$CONFIG" main &
disown
