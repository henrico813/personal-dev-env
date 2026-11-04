#!/bin/bash
# Shows a window that an i3 rule hid in the scratchpad, once it has drawn.
# A new window shows leftover screen until its first draw, about 1.3 s for
# GNOME Settings, so i3/config hides these windows until this script runs.
# Example: show-when-drawn.sh gnome-control-center
set -uo pipefail

# Prints the X window id of a scratchpad window whose class is argv[1].
hidden_window='
import json, sys
def find(n, hidden=False):
    hidden = hidden or n.get("name") == "__i3_scratch"
    if hidden and (n.get("window_properties") or {}).get("class", "").lower() == sys.argv[1]:
        print(n["window"])
        sys.exit()
    for c in n["nodes"] + n["floating_nodes"]:
        find(c, hidden)
find(json.load(sys.stdin))'

# Wait up to about 15 s for the window, since a first start can be slow.
tries=300
id=""
while [[ -z $id ]] && ((tries-- > 0)); do
    sleep 0.05
    id=$(i3-msg -t get_tree | python3 -c "$hidden_window" "$1")
done
[[ -n $id ]] || exit 0

xprop -id "$id" -f _NET_WM_WINDOW_OPACITY 32c -set _NET_WM_WINDOW_OPACITY 0
i3-msg "[id=$id] scratchpad show, move position center" >/dev/null
# GNOME Settings sets the opaque region on its first draw, so wait up to about
# 5 s for it. Xfce's GTK 3 Displays sets it before mapping, so it gets a longer
# fixed wait instead.
if [[ $1 == gnome-control-center ]]; then
    tries=100
    while ((tries-- > 0)) && [[ $(xprop -id "$id" _NET_WM_OPAQUE_REGION) != *"= "[0-9]* ]]; do
        sleep 0.05
    done
    sleep 0.2
else
    sleep 0.5
fi
xprop -id "$id" -f _NET_WM_WINDOW_OPACITY 32c -set _NET_WM_WINDOW_OPACITY 4294967295
