#!/bin/bash
# Cava visualizer for polybar
# Converts cava raw output to Unicode bar characters

bar_chars="▁▂▃▄▅▆▇█"
dict="s/;//g;"

for i in $(seq 0 7); do
    dict="${dict}s/$i/${bar_chars:$i:1}/g;"
done

cava -p ~/.config/cava/config | sed -u "$dict"
