#!/bin/bash
# Prints the clocks listed in ~/.config/pde-desktop/world-clocks, which
# pde-installer never writes. Each line is "LABEL ZONE [PLACE]": ZONE comes from
# timedatectl list-timezones, or is - for no time, and PLACE is a wttr.in
# location for the weather. Prints nothing when the file is missing.

file="$HOME/.config/pde-desktop/world-clocks"
[[ -r "$file" ]] || exit 0

output=""
while read -r label zone place _ || [[ -n "$label" ]]; do
    if [[ -z "$label" || "$label" == \#* ]]; then
        continue
    fi
    clock="$label"
    if [[ -n "$zone" && "$zone" != "-" ]]; then
        clock+=" $(TZ="$zone" date +%H:%M)"
    fi
    if [[ -n "$place" ]]; then
        clock+=" $(curl -s --max-time 2 "wttr.in/$place?m&format=%c%t" 2>/dev/null)"
    fi
    output+="${output:+  }$clock"
done <"$file"
echo "$output"
