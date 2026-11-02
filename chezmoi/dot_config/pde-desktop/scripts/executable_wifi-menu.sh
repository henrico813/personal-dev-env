#!/bin/bash
# Picks a Wi-Fi network in rofi and connects to it with NetworkManager.
# A saved or open network connects at once; a new secured one asks for its
# password.
set -uo pipefail

# One SECURITY:SSID line per network. The SSID comes last, so a colon in it
# stays part of the name.
networks=$(nmcli --escape no -t -f SECURITY,SSID device wifi list --rescan yes 2>/dev/null |
    awk '{ name = substr($0, index($0, ":") + 1) } name != "" && !seen[name]++')
if [[ -z "$networks" ]]; then
    notify-send "Wi-Fi" "No networks found"
    exit 0
fi

ssid=$(cut -d: -f2- <<<"$networks" | rofi -dmenu -i -p "Wi-Fi") || exit 0
[[ -n "$ssid" ]] || exit 0
# ENVIRON keeps backslashes in the name, which awk -v would rewrite. A name
# typed in that is not listed, such as a hidden network, gets "unknown".
security=$(ssid="$ssid" awk '
    substr($0, index($0, ":") + 1) == ENVIRON["ssid"] { print substr($0, 1, index($0, ":") - 1); found = 1; exit }
    END { if (!found) print "unknown" }
' <<<"$networks")

if nmcli --escape no -t -f NAME connection show | grep -Fxq -- "$ssid"; then
    result=$(nmcli connection up id "$ssid" 2>&1)
elif [[ -z "$security" || "$security" == "--" ]]; then
    result=$(nmcli device wifi connect "$ssid" 2>&1)
else
    password=$(rofi -dmenu -password -p "Password for $ssid" </dev/null) || exit 0
    # --ask reads the password from stdin, so it never appears in ps.
    result=$(printf '%s\n' "$password" | nmcli --ask device wifi connect "$ssid" 2>&1)
fi
status=$?

if [[ $status -eq 0 ]]; then
    notify-send "Wi-Fi" "Connected to $ssid"
else
    notify-send -u critical "Wi-Fi" "Could not connect to $ssid: ${result##*: }"
fi
