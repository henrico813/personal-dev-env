#!/usr/bin/env bash
# Installs the Neovim release pde-installer pins, so CI tests the version
# users get. The version and checksum are read from the installer's pins, so
# changing them there changes CI too.
# Usage: pde-installer/test/install-pinned-nvim.sh DIR
# Prints the folder that holds nvim, for adding to PATH.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dir="${1:?usage: install-pinned-nvim.sh DIR}"
[ "$(uname -m)" = x86_64 ] || { echo "only x86_64 is supported" >&2; exit 1; }

# The first Version after the neovim name, on its line or a later one.
version="$(awk '/Name: *"neovim"/ { neovim = 1 }
	neovim && /Version:/ { sub(/.*Version: *"/, ""); sub(/".*/, ""); print; exit }' "$root/internal/manifest/manifest.go")"
# The neovim checksum inside the amd64 case of toolsForPlatform.
sha="$(awk '/case "amd64":/ { amd64 = 1 } /case "arm64":/ { amd64 = 0 }
	amd64 && /"neovim":/ { gsub(/[",]/, "", $2); print $2 }' "$root/internal/direct/tools.go")"
if [ -z "$version" ] || [ "${#sha}" -ne 64 ]; then
	echo "cannot read the Neovim pin from manifest.go and tools.go" >&2
	exit 1
fi

archive="nvim-linux-x86_64.tar.gz"
mkdir -p "$dir"
curl -fsSL -o "$dir/$archive" "https://github.com/neovim/neovim/releases/download/v$version/$archive"
echo "$sha  $dir/$archive" | sha256sum -c --quiet -
tar -xzf "$dir/$archive" -C "$dir"
"$dir/nvim-linux-x86_64/bin/nvim" --version | head -n 1 >&2
echo "$dir/nvim-linux-x86_64/bin"
