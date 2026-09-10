#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
[[ "$(id -u)" -ne 0 ]]
[[ -x /usr/bin/sudo ]]

for package in build-essential bzip2 fontconfig gawk make patch python3; do
	if dpkg-query -W -f='${db:Status-Abbrev}' "$package" 2>/dev/null | grep -q '^ii'; then
		printf 'full-only package unexpectedly installed: %s\n' "$package" >&2
		exit 1
	fi
done

pde-installer install --profile terminal --repo-root "$REPO_ROOT"
[[ -f "$HOME/.config/pde/config.json" ]]
grep -Eq '"profile"[[:space:]]*:[[:space:]]*"terminal"' "$HOME/.config/pde/config.json"

[[ "$(tmux -V)" == 'tmux 3.7b' ]]
tmux_binary="$HOME/.local/share/pde/tmux/3.7b/bin/tmux"
[[ -x "$tmux_binary" ]]
file "$tmux_binary" | grep -qi static

pde-installer update --repo-root "$REPO_ROOT"
pde-installer config --repo-root "$REPO_ROOT"
pde-installer doctor --repo-root "$REPO_ROOT"

inventory="$(pde-installer list --repo-root "$REPO_ROOT")"
for item in zsh git xclip unzip tmux aqua fd fzf ripgrep bat jq chezmoi eza zoxide bottom yq yazi ya repository-config antidote tpm ohmyzsh powerlevel10k zsh-z zsh-autosuggestions zsh-completions zsh-syntax-highlighting zsh-history-substring-search tmux-sensible tmux-resurrect; do
	grep -q $'\t'"$item"$'\t.*\tcurrent$' <<<"$inventory"
done
for item in build-essential gopls lua-language-server opencode-ai '@openai/codex' '@earendil-works/pi-coding-agent' planner blink.cmp FiraCode neovim go rust node keychain; do
	! grep -q $'\t'"$item"$'\t' <<<"$inventory"
done

for path in "$HOME/.config/nvim" "$HOME/.config/alacritty" "$HOME/.config/wezterm" "$HOME/.config/opencode" "$HOME/.codex" "$HOME/.agents" "$HOME/.pi"; do
	[[ ! -e "$path" ]]
done
mapfile -t aqua_files < <(find "$HOME/.config/aquaproj-aqua" -maxdepth 1 -type f -printf '%f\n' | sort)
[[ "${aqua_files[*]}" == 'aqua-terminal-checksums.json aqua-terminal.yaml' ]]
for launcher in go node npm nvim; do
	[[ ! -e "$HOME/.local/bin/$launcher" ]]
	! command -v "$launcher" >/dev/null 2>&1
done
for path in "$HOME/.local/share/antidote" "$HOME/.local/share/zsh/plugins/ohmyzsh" "$HOME/.local/share/zsh/plugins/powerlevel10k" "$HOME/.local/share/zsh/plugins/zsh-completions" "$HOME/.tmux/plugins/tpm" "$HOME/.tmux/plugins/tmux-sensible" "$HOME/.tmux/plugins/tmux-resurrect"; do
	[[ -e "$path" ]]
done
! grep -q 'keychain' "$HOME/.zshrc"
! grep -q '@resurrect-strategy-nvim' "$HOME/.tmux.conf"
! grep -q 'nvim' "$HOME/.tmux.conf"
zsh -n "$HOME/.zshrc"

printf 'PASS: terminal installation on %s\n' "$(. /etc/os-release && printf '%s' "$PRETTY_NAME")"
