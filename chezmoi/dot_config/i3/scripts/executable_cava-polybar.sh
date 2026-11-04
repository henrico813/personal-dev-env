#!/bin/bash
# Cava visualizer for polybar
# Converts cava raw output to Unicode bar characters

bar_chars="▁▂▃▄▅▆▇█"
dict="s/;//g;"

for i in $(seq 0 7); do
    dict="${dict}s/$i/${bar_chars:$i:1}/g;"
done

# Only raw frames (digits and semicolons) reach the bar, never cava's errors.
cava -p ~/.config/cava/config 2>/dev/null | sed -u -n "/^[0-7;]*\$/{${dict}p}"
