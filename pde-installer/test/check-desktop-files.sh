#!/usr/bin/env bash
# Checks the desktop configs and scripts in chezmoi/ without a display. It
# needs i3-wm and shellcheck from apt.
set -euo pipefail

SOURCE="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../chezmoi" && pwd)"
home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT

# The config includes ~/.config/i3/local.conf, so both go in a scratch HOME.
mkdir -p "$home/.config/i3"
cp "$SOURCE/dot_config/i3/config" "$home/.config/i3/config"
cp "$SOURCE/dot_config/i3/create_local.conf" "$home/.config/i3/local.conf"
HOME="$home" i3 -C -c "$home/.config/i3/config"

# Syntax errors fail shellcheck too. polypomo is Python and is skipped.
scripts=()
while IFS= read -r -d '' script; do
	if head -n 1 "$script" | grep -q bash; then
		scripts+=("$script")
	fi
done < <(find "$SOURCE/dot_config/pde-desktop" "$SOURCE/dot_config/i3" "$SOURCE/dot_config/polybar" \
	-type f -name 'executable_*' -print0)
shellcheck "${scripts[@]}"
