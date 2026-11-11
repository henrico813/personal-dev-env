#!/usr/bin/env bash
# Installs pdev inside WSL Ubuntu from the checkout this script sits in, then
# checks the result. The Windows guide in pde-installer/docs/how-to says what
# to do before running it. Usage is bash install/wsl.sh terminal|client|full.
set -euo pipefail

profile="${1:-}"
case "$profile" in
terminal | client | full) ;;
*)
	echo "usage: bash install/wsl.sh terminal|client|full" >&2
	exit 2
	;;
esac

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# A checkout on a Windows drive is slow, and one made by Windows Git has CRLF
# line endings that break the chezmoi scripts.
case "$repo" in
/mnt/*)
	echo "pdev: $repo is on a Windows drive; clone it inside WSL, such as in ~/Projects" >&2
	exit 1
	;;
esac

# Checked first so a missing email does not stop the install halfway.
if [ "$profile" != terminal ] && [ -z "$(git config --global user.email || true)" ]; then
	echo "pdev: $profile needs a Git email; run git config --global user.email you@example.com" >&2
	exit 1
fi

if ! command -v go >/dev/null; then
	sudo apt-get update
	sudo apt-get install -y golang-go
fi

mkdir -p "$HOME/.local/bin"
go build -C "$repo/pde-installer" -o "$HOME/.local/bin/pde-installer" .
export PATH="$HOME/.local/bin:$PATH"

# A failed install still runs doctor, so the user sees what is missing.
ok=true
pde-installer install "$profile" --repo-root "$repo" || ok=false

# The pdev shell config is for zsh, which the install puts in place.
zsh_path="$(command -v zsh || true)"
if [ -n "$zsh_path" ] && [ "$(getent passwd "$(id -un)" | cut -d: -f7)" != "$zsh_path" ]; then
	sudo usermod --shell "$zsh_path" "$(id -un)"
fi

pde-installer doctor --repo-root "$repo" || ok=false
if "$ok"; then
	echo "pdev install ok; open a new terminal to start using it"
else
	echo "pdev install incomplete; fix the error lines above, then run this script again" >&2
	exit 1
fi
