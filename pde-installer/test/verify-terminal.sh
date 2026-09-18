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

pde-installer install terminal --repo-root "$REPO_ROOT"
assert_absent_packages
[[ -f "$HOME/.config/pde/config.json" ]]
jq -e '.profile == "terminal" and .color_profile == "tokyo-night"' "$HOME/.config/pde/config.json" >/dev/null
grep -Fq 'export BAT_THEME="TwoDark"' "$HOME/.zshrc"
grep -Fq '#7aa2f7' "$HOME/.tmux.conf"
grep -Fq '#7aa2f7' "$HOME/.p10k.zsh"
bat --list-themes | grep -Fxq TwoDark

before_rollback="$(sha256sum "$HOME/.config/pde/config.json" "$HOME/.zshrc" "$HOME/.tmux.conf" "$HOME/.p10k.zsh")"
zsh_template="$REPO_ROOT/chezmoi/dot_zshrc.tmpl"
template_backup="$(mktemp)"
cp "$zsh_template" "$template_backup"
printf '\n{{ fail "rollback test" }}\n' >>"$zsh_template"
if pde-installer install --color-profile gruvbox-dark --repo-root "$REPO_ROOT"; then
	rollback_status=0
else
	rollback_status=$?
fi
cp "$template_backup" "$zsh_template"
rm "$template_backup"
[[ "$rollback_status" -ne 0 ]]
[[ "$(sha256sum "$HOME/.config/pde/config.json" "$HOME/.zshrc" "$HOME/.tmux.conf" "$HOME/.p10k.zsh")" == "$before_rollback" ]]

pde-installer install --color-profile everforest-dark --repo-root "$REPO_ROOT"
jq -e '.profile == "terminal" and .color_profile == "everforest-dark"' "$HOME/.config/pde/config.json" >/dev/null
grep -Fq 'export BAT_THEME="zenburn"' "$HOME/.zshrc"
grep -Fq '#7fbbb3' "$HOME/.zshrc"
grep -Fq '#7fbbb3' "$HOME/.tmux.conf"
grep -Fq '#7fbbb3' "$HOME/.p10k.zsh"
bat --list-themes | grep -Fxq zenburn
zsh -c 'source "$1"; printf "item\n" | fzf --filter=item >/dev/null' zsh "$HOME/.zshrc"
zsh -n "$HOME/.zshrc"
zsh -n "$HOME/.p10k.zsh"
tmux -L pde-color -f "$HOME/.tmux.conf" new-session -d -s config-check
[[ "$(tmux -L pde-color show-option -gv pane-active-border-style)" == 'fg=#7fbbb3' ]]
tmux -L pde-color kill-server
! grep -Eq '](4|10|11|12);' "$HOME/.zshrc" "$HOME/.p10k.zsh" "$HOME/.tmux.conf"

rm -rf "$HOME/.local/share/aquaproj-aqua"
aqua_root="$HOME/.local/share/aquaproj-aqua"
full_only_package="$aqua_root/pkgs/go.dev/gopls/v0.23.0/gopls"
full_only_launcher="$aqua_root/bin/gopls"
mkdir -p "$HOME/.config/pde" "$(dirname "$full_only_package")" "$(dirname "$full_only_launcher")"
printf '{"profile":"full","color_profile":"gruvbox-dark"}\n' >"$HOME/.config/pde/config.json"
printf '#!/bin/sh\nprintf '\''gopls-retained\\n'\''\n' >"$full_only_package"
chmod 0755 "$full_only_package"
ln -s "$full_only_package" "$full_only_launcher"
mkdir -p "$HOME/.config/nvim" "$HOME/.agents" "$HOME/.codex"
printf 'retain-opencode\n' >"$HOME/.config/opencode"
printf 'retain-nvim\n' >"$HOME/.config/nvim/init.lua"
printf 'retain-agents\n' >"$HOME/.agents/marker"
printf 'retain-codex\n' >"$HOME/.codex/marker"
rm "$HOME/.tmux.conf"

pde-installer install terminal --repo-root "$REPO_ROOT"
assert_absent_packages
[[ -f "$HOME/.config/pde/config.json" ]]
jq -e '.profile == "terminal" and .color_profile == "gruvbox-dark"' "$HOME/.config/pde/config.json" >/dev/null
grep -Fq 'export BAT_THEME="gruvbox-dark"' "$HOME/.zshrc"
grep -Fq '#83a598' "$HOME/.tmux.conf"
grep -Fq '#83a598' "$HOME/.p10k.zsh"
bat --list-themes | grep -Fxq gruvbox-dark
[[ "$("$full_only_launcher")" == "gopls-retained" ]]
[[ -L "$full_only_launcher" ]]
[[ -x "$full_only_package" ]]
[[ "$(cat "$HOME/.config/opencode")" == "retain-opencode" ]]
[[ "$(cat "$HOME/.config/nvim/init.lua")" == "retain-nvim" ]]
[[ "$(cat "$HOME/.agents/marker")" == "retain-agents" ]]
[[ "$(cat "$HOME/.codex/marker")" == "retain-codex" ]]

[[ "$(tmux -V)" == 'tmux 3.7b' ]]
tmux_binary="$HOME/.local/share/pde/tmux/3.7b/bin/tmux"
[[ -x "$tmux_binary" ]]
file "$tmux_binary" | grep -qi static

pde-installer install --repo-root "$REPO_ROOT"
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

for path in "$HOME/.config/alacritty" "$HOME/.config/wezterm" "$HOME/.pi"; do
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
