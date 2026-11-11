#!/usr/bin/env bash
# Installs pdev on an Apple Silicon Mac, then checks the result. The macOS
# guide in pde-installer/docs/how-to says what to do first.
# Usage is bash install/macos.sh [terminal|client], and client is the default.
# PDE_DIR picks the checkout, and PDE_NO_PULL=1 installs it as it is, as CI does.
# Runs under the bash 3.2 that macOS ships.
set -euo pipefail

profile="${1:-client}"
case "$profile" in
terminal | client) ;;
*)
	echo "usage: bash install/macos.sh [terminal|client]" >&2
	exit 2
	;;
esac

# Each check runs before anything installs, so a missing step stops early.
if [ "$(uname -s)" != Darwin ] || [ "$(uname -m)" != arm64 ]; then
	echo "pdev: this script needs macOS on an Apple Silicon Mac" >&2
	exit 1
fi
if ! xcode-select -p >/dev/null 2>&1; then
	echo "pdev: install the Xcode Command Line Tools first, with: xcode-select --install" >&2
	exit 1
fi
brew=/opt/homebrew/bin/brew
if [ ! -x "$brew" ]; then
	echo "pdev: install Homebrew first. The command from https://brew.sh is:" >&2
	# shellcheck disable=SC2016 # The user runs this command, so it must not expand here.
	echo '/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"' >&2
	exit 1
fi
if [ "$profile" = client ] && [ -z "$(git config --global user.email || true)" ]; then
	echo "pdev: client needs a Git email; run git config --global user.email you@example.com" >&2
	exit 1
fi

eval "$("$brew" shellenv)"
# git comes from the Command Line Tools, and the installer adds Homebrew's.
command -v go >/dev/null || "$brew" install go

dir="${PDE_DIR:-$HOME/Projects/personal-dev-env}"
if [ ! -d "$dir/.git" ]; then
	mkdir -p "$(dirname "$dir")"
	git clone https://github.com/henrico813/personal-dev-env.git "$dir"
elif [ "${PDE_NO_PULL:-}" != 1 ]; then
	git -C "$dir" pull --ff-only
fi

mkdir -p "$HOME/.local/bin"
go build -C "$dir/pde-installer" -o "$HOME/.local/bin/pde-installer" .
export PATH="$HOME/.local/bin:$PATH"

pde-installer install "$profile" --repo-root "$dir"

# doctor prints an error line for each problem. list marks each item, and only
# current and installed are good.
ok=true
pde-installer doctor --repo-root "$dir" || ok=false
items="$(pde-installer list --repo-root "$dir")" || ok=false
not_ready="$(printf '%s\n' "$items" | awk -F '\t' 'NR > 1 && $5 != "current" && $5 != "installed"')"
if [ -n "$not_ready" ]; then
	echo "pdev: these items are not ready (owner, item, wanted, installed, state):" >&2
	printf '%s\n' "$not_ready" >&2
	ok=false
fi

if [ "$ok" = true ]; then
	echo "pdev install ok; open a new terminal to start using it"
else
	echo "pdev install incomplete; fix the lines above, then run this script again" >&2
	exit 1
fi
