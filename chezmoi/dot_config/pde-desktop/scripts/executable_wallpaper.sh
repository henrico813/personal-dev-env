#!/bin/bash
# Plays the first .mp4, then the first .webm, in ~/.config/wallpapers as the
# wallpaper on every output: mpvpaper on Sway, xwinwrap with mpv on i3. Without
# a video, or without the player pde-installer built and installed in
# ~/.local/bin, it shows Ubuntu's default image.

WALLPAPER_DIR="$HOME/.config/wallpapers"
FALLBACK_IMAGE=/usr/share/backgrounds/warty-final-ubuntu.png
# The session PATH may lack ~/.local/bin, so the players are called by path.
MPVPAPER="$HOME/.local/bin/mpvpaper"
XWINWRAP="$HOME/.local/bin/xwinwrap"

find_video() {
    local file
    for file in "$WALLPAPER_DIR"/*.mp4 "$WALLPAPER_DIR"/*.webm; do
        if [[ -f "$file" ]]; then
            echo "$file"
            return 0
        fi
    done
    return 1
}

play_on_wayland() {
    local video="$1" output
    for output in $(swaymsg -t get_outputs | jq -r '.[].name'); do
        "$MPVPAPER" -o "loop=yes ao=null" "$output" "$video" &
    done
}

play_on_x() {
    local video="$1" geometry
    for geometry in $(xrandr --query | grep ' connected' | grep -oE '[0-9]+x[0-9]+\+[0-9]+\+[0-9]+'); do
        "$XWINWRAP" -g "$geometry" -ov -ni -s -nf -b -d -- \
            mpv -wid WID --loop --no-audio --no-osc --panscan=1.0 \
            --no-cache --hwdec=auto \
            --demuxer-max-bytes=50MiB --demuxer-max-back-bytes=10MiB \
            "$video"
    done
}

warn_missing() {
    local video="$1" player="$2"
    if [[ -n "$video" ]]; then
        echo "wallpaper.sh: $player is missing, so showing $FALLBACK_IMAGE" >&2
    fi
}

video=$(find_video)
if [[ -n "${WAYLAND_DISPLAY:-}" ]]; then
    pkill -x swaybg
    pkill -x mpvpaper
    if [[ -n "$video" && -x "$MPVPAPER" ]]; then
        play_on_wayland "$video"
    else
        warn_missing "$video" "$MPVPAPER"
        swaybg -i "$FALLBACK_IMAGE" -m fill &
    fi
else
    pkill -x xwinwrap
    if [[ -n "$video" && -x "$XWINWRAP" ]]; then
        play_on_x "$video"
    else
        warn_missing "$video" "$XWINWRAP"
        feh --bg-fill "$FALLBACK_IMAGE"
    fi
fi
