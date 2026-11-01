#!/bin/bash
# Adjust volume and show notification with progress bar
# Usage: volume-notify.sh <up|down|mute>

case "$1" in
    up)   pactl set-sink-volume @DEFAULT_SINK@ +5% ;;
    down) pactl set-sink-volume @DEFAULT_SINK@ -5% ;;
    mute) pactl set-sink-mute @DEFAULT_SINK@ toggle ;;
esac

MUTED=$(pactl get-sink-mute @DEFAULT_SINK@ | grep -o "yes\|no")
VOLUME=$(pactl get-sink-volume @DEFAULT_SINK@ | grep -o '[0-9]*%' | head -1 | tr -d '%')

if [ "$MUTED" = "yes" ]; then
    dunstify -a "volume" -u low -r 9998 -h int:value:0 "Volume: Muted"
else
    dunstify -a "volume" -u low -r 9998 -h int:value:"$VOLUME" "Volume: ${VOLUME}%"
fi
