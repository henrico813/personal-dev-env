#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
[[ "$(id -u)" -ne 0 ]]
[[ -x /usr/bin/sudo ]]

full_only_packages=(build-essential bison bzip2 libevent-dev libncurses-dev make patch pkg-config python3)
assert_absent_packages() {
	local package
	for package in "${full_only_packages[@]}"; do
		if dpkg-query -W -f='${db:Status-Abbrev}' "$package" 2>/dev/null | grep -Fq 'ii'; then
			printf 'full-only package unexpectedly installed: %s\n' "$package" >&2
			exit 1
		fi
	done
}
assert_absent_packages

pde-installer install --profile terminal --repo-root "$REPO_ROOT"
assert_absent_packages
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
expected_items='zsh git xclip unzip tmux aqua fd fzf ripgrep bat jq chezmoi eza zoxide bottom yq yazi ya repository-config antidote tpm ohmyzsh powerlevel10k zsh-z zsh-autosuggestions zsh-completions zsh-syntax-highlighting zsh-history-substring-search tmux-sensible tmux-resurrect'
actual_items="$(awk -F '\t' 'NR > 1 { print $2 }' <<<"$inventory" | paste -sd ' ' -)"
[[ "$actual_items" == "$expected_items" ]]
item_status() {
	local item="$1" status="$2"
	awk -F '\t' -v item="$item" -v status="$status" 'NR > 1 && $2 == item && $5 == status { found=1 } END { exit !found }' <<<"$inventory"
}
for item in zsh git xclip unzip; do
	item_status "$item" installed
done
for item in tmux aqua fd fzf ripgrep bat jq chezmoi eza zoxide bottom yq yazi ya repository-config antidote tpm ohmyzsh powerlevel10k zsh-z zsh-autosuggestions zsh-completions zsh-syntax-highlighting zsh-history-substring-search tmux-sensible tmux-resurrect; do
	item_status "$item" current
done
for item in build-essential bison gopls lua-language-server opencode-ai '@openai/codex' '@earendil-works/pi-coding-agent' planner blink.cmp FiraCode JetBrainsMono neovim go rust node keychain; do
	! awk -F '\t' -v item="$item" 'NR > 1 && $2 == item { found=1 } END { exit found }' <<<"$inventory"
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
for path in "$HOME/.local/share/antidote" "$HOME/.local/share/zsh/plugins/ohmyzsh" "$HOME/.local/share/zsh/plugins/powerlevel10k" "$HOME/.local/share/zsh/plugins/zsh-z" "$HOME/.local/share/zsh/plugins/zsh-autosuggestions" "$HOME/.local/share/zsh/plugins/zsh-completions" "$HOME/.local/share/zsh/plugins/zsh-syntax-highlighting" "$HOME/.local/share/zsh/plugins/zsh-history-substring-search" "$HOME/.tmux/plugins/tpm" "$HOME/.tmux/plugins/tmux-sensible" "$HOME/.tmux/plugins/tmux-resurrect"; do
	[[ -e "$path" ]]
done
! grep -q 'keychain' "$HOME/.zshrc"
! grep -q '@resurrect-strategy-nvim' "$HOME/.tmux.conf"
! grep -q 'nvim' "$HOME/.tmux.conf"
zsh -n "$HOME/.zshrc"

printf 'PASS: terminal installation on %s\n' "$(. /etc/os-release && printf '%s' "$PRETTY_NAME")"
