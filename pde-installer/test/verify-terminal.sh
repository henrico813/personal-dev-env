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
export AQUA_ROOT_DIR="$HOME/.local/share/aquaproj-aqua"
export AQUA_GLOBAL_CONFIG="$HOME/.config/aquaproj-aqua/aqua-terminal.yaml"
export AQUA_CHECKSUMS_PATH="$HOME/.config/aquaproj-aqua/aqua-terminal-checksums.json"
export PATH="$AQUA_ROOT_DIR/bin:$PATH"
assert_absent_packages
[[ -f "$HOME/.config/pde/config.json" ]]
jq -e '.profile == "terminal" and .color_profile == "tokyo-night"' "$HOME/.config/pde/config.json" >/dev/null
grep -Fq 'export BAT_THEME="TwoDark"' "$HOME/.zshrc"
grep -Fq '#7aa2f7' "$HOME/.tmux.conf"
grep -Fq '#7aa2f7' "$HOME/.p10k.zsh"
bat --list-themes | grep -Fxq TwoDark

tui_home="$(mktemp -d)"
mkdir -p "$tui_home/.config/opencode"
cat >"$tui_home/.config/opencode/tui.jsonc" <<'EOF'
{
  // Existing OpenCode TUI settings remain user-owned.
  "mouse": false,
  "scroll_speed": 2,
}
EOF
render_opencode_tui() {
	local profile="$1"
	PDE_PROFILE=full \
	PDE_COLOR_PROFILE="$profile" \
	PDE_REPO_ROOT="$REPO_ROOT" \
	PDE_SURVEIL_STATE_PATTERN="$tui_home/.local/state/surveil/**" \
	chezmoi \
		--config /dev/null \
		--config-format toml \
		--source "$REPO_ROOT/chezmoi" \
		--destination "$tui_home" \
		--persistent-state "$tui_home/chezmoi.boltdb" \
		--color false \
		--refresh-externals=never \
		cat "$tui_home/.config/opencode/tui.jsonc"
}
for mapping in tokyo-night:tokyonight everforest-dark:everforest gruvbox-dark:gruvbox; do
	profile="${mapping%%:*}"
	theme="${mapping#*:}"
	rendered="$(render_opencode_tui "$profile")"
	jq -e --arg theme "$theme" \
		'.theme == $theme and .mouse == false and .scroll_speed == 2' \
		<<<"$rendered" >/dev/null
done
printf '[]\n' >"$tui_home/.config/opencode/tui.jsonc"
if render_opencode_tui everforest-dark >/dev/null 2>&1; then
	printf 'non-object OpenCode TUI config unexpectedly rendered\n' >&2
	exit 1
fi
grep -Fxq '[]' "$tui_home/.config/opencode/tui.jsonc"
rm "$tui_home/.config/opencode/tui.jsonc"
rendered="$(render_opencode_tui everforest-dark)"
jq -e '. == {"theme":"everforest"}' <<<"$rendered" >/dev/null
rm -rf "$tui_home"

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
[[ ! -e "$HOME/.config/herdr" ]]

rm -rf "$HOME/.local/share/aquaproj-aqua"
aqua_root="$HOME/.local/share/aquaproj-aqua"
full_only_package="$aqua_root/pkgs/go.dev/gopls/v0.23.0/gopls"
full_only_launcher="$aqua_root/bin/gopls"
mkdir -p "$HOME/.config/pde" "$(dirname "$full_only_package")" "$(dirname "$full_only_launcher")"
printf '{"profile":"full","color_profile":"gruvbox-dark"}\n' >"$HOME/.config/pde/config.json"
printf '#!/bin/sh\nprintf '\''gopls-retained\\n'\''\n' >"$full_only_package"
chmod 0755 "$full_only_package"
ln -s "$full_only_package" "$full_only_launcher"
mkdir -p "$HOME/.config/nvim" "$HOME/.agents" "$HOME/.codex" "$HOME/.config/herdr"
printf 'retain-opencode\n' >"$HOME/.config/opencode"
printf 'retain-nvim\n' >"$HOME/.config/nvim/init.lua"
printf 'retain-agents\n' >"$HOME/.agents/marker"
printf 'retain-codex\n' >"$HOME/.codex/marker"
rm "$HOME/.tmux.conf"
printf 'retain-herdr\n' >"$HOME/.config/herdr/config.toml"

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
[[ "$(cat "$HOME/.config/herdr/config.toml")" == "retain-herdr" ]]

[[ "$(tmux -V)" == 'tmux 3.7b' ]]
tmux_binary="$HOME/.local/share/pde/tmux/3.7b/bin/tmux"
[[ -x "$tmux_binary" ]]
file "$tmux_binary" | grep -qi static
tmux_socket="pde-terminal-$$"
trap 'tmux -L "$tmux_socket" kill-server 2>/dev/null || true' EXIT
workspace="$HOME/tw-workspace"
mkdir -p "$workspace"
tm_workspace="$HOME/tm-workspace"
mkdir -p "$tm_workspace"
if PATH="$HOME/.local/bin:/usr/bin:/bin" zsh -ic "tm --no-attach '$tm_workspace'" 2>"$tm_workspace/no-herdr-error"; then
	printf 'tm unexpectedly started without Herdr\n' >&2
	exit 1
fi
grep -Fq 'Error: tm requires Herdr' "$tm_workspace/no-herdr-error"

cat >"$HOME/.local/bin/herdr" <<'SCRIPT'
#!/usr/bin/env bash
exit 1
SCRIPT
chmod 755 "$HOME/.local/bin/herdr"

tm_hash="$(printf '%s' "$tm_workspace" | sha256sum)"
tm_hash="${tm_hash%% *}"
tm_scope="tm-${tm_workspace##*/}-${tm_hash:0:8}"
tmux -L "$tmux_socket" -f "$HOME/.tmux.conf" new-session -d -s controller -n shell -c "$tm_workspace" \
	"zsh -ic 'if tm --no-attach \"$tm_workspace\"; then print 0 >\"$tm_workspace/start-failure-status\"; else print 1 >\"$tm_workspace/start-failure-status\"; fi; tmux wait-for -S tm-start-failure; exec zsh'"
timeout 20 tmux -L "$tmux_socket" wait-for tm-start-failure
[[ "$(cat "$tm_workspace/start-failure-status")" == 1 ]]
for role in hub pocket dash; do
	tmux -L "$tmux_socket" kill-session -t "${tm_scope}-${role}" 2>/dev/null || true
done

cat >"$HOME/.local/bin/herdr" <<'SCRIPT'
#!/usr/bin/env bash
printf 'config=%s args=%s\n' "${HERDR_CONFIG_PATH:-}" "$*" >>"$HOME/herdr-invocations"
exec sleep 300
SCRIPT
chmod 755 "$HOME/.local/bin/herdr"
: >"$HOME/herdr-invocations"

controller_pane="$(tmux -L "$tmux_socket" display-message -p -t controller:shell '#{pane_id}')"
tmux -L "$tmux_socket" send-keys -t "$controller_pane" "tm --no-attach '$tm_workspace'; print \$? >'$tm_workspace/status'; tmux wait-for -S tm-ready" C-m
timeout 20 tmux -L "$tmux_socket" wait-for tm-ready
if [[ "$(cat "$tm_workspace/status")" != 0 ]]; then
	tmux -L "$tmux_socket" list-sessions >&2
	tmux -L "$tmux_socket" list-windows -t "${tm_scope}-dash" >&2 || true
	tmux -L "$tmux_socket" capture-pane -p -t "${tm_scope}-dash:bootstrap" >&2 || true
	tmux -L "$tmux_socket" capture-pane -p -t controller:shell >&2
	exit 1
fi
expected_sessions="$(printf '%s\n' controller "${tm_scope}-dash" "${tm_scope}-hub" "${tm_scope}-pocket" | sort)"
[[ "$(tmux -L "$tmux_socket" list-sessions -F '#{session_name}' | sort)" == "$expected_sessions" ]]
[[ "$(tmux -L "$tmux_socket" list-windows -t "${tm_scope}-hub" -F '#{window_name}' | sort)" == $'herdr\nshell' ]]
[[ "$(tmux -L "$tmux_socket" list-windows -t "${tm_scope}-pocket" -F '#{window_name}' | sort)" == $'herdr\nshell' ]]
[[ "$(tmux -L "$tmux_socket" list-windows -t "${tm_scope}-dash" -F '#{window_name}:#{window_panes}')" == 'main:4' ]]
timeout 20 bash -c 'until [[ $(wc -l <"$1") -eq 2 ]]; do sleep 0.1; done' _ "$HOME/herdr-invocations"
[[ "$(sort "$HOME/herdr-invocations")" == $'config= args=--session default\nconfig= args=--session default' ]]
for role in hub pocket dash; do
	[[ "$(tmux -L "$tmux_socket" show-option -qv -t "${tm_scope}-${role}" @tm-root)" == "$tm_workspace" ]]
done
hub_pane="$(tmux -L "$tmux_socket" display-message -p -t "${tm_scope}-hub:herdr" '#{pane_id}')"
tmux -L "$tmux_socket" resize-window -t "${tm_scope}-hub:herdr" -x 120 -y 40
tmux -L "$tmux_socket" resize-window -t "${tm_scope}-pocket:herdr" -x 46 -y 22
[[ "$(tmux -L "$tmux_socket" display-message -p -t "${tm_scope}-hub:herdr" '#{window_width}x#{window_height}')" == 120x40 ]]
[[ "$(tmux -L "$tmux_socket" display-message -p -t "${tm_scope}-pocket:herdr" '#{window_width}x#{window_height}')" == 46x22 ]]
tmux -L "$tmux_socket" send-keys -t "$controller_pane" "tm --no-attach '$tm_workspace'; print \$? >'$tm_workspace/repeat-status'; tmux wait-for -S tm-repeat" C-m
timeout 20 tmux -L "$tmux_socket" wait-for tm-repeat
[[ "$(cat "$tm_workspace/repeat-status")" == 0 ]]
[[ "$(tmux -L "$tmux_socket" display-message -p -t "${tm_scope}-hub:herdr" '#{pane_id}')" == "$hub_pane" ]]

notes_pane="$(tmux -L "$tmux_socket" new-window -d -t "${tm_scope}-hub" -n notes -c "$tm_workspace" -P -F '#{pane_id}' 'exec sleep 300')"
tmux -L "$tmux_socket" kill-window -t "${tm_scope}-hub:shell"
tmux -L "$tmux_socket" kill-window -t "${tm_scope}-pocket:herdr"
tmux -L "$tmux_socket" send-keys -t "$controller_pane" "tm --no-attach '$tm_workspace'; print \$? >'$tm_workspace/repair-status'; tmux wait-for -S tm-repair" C-m
timeout 20 tmux -L "$tmux_socket" wait-for tm-repair
[[ "$(cat "$tm_workspace/repair-status")" == 0 ]]
timeout 20 bash -c 'until [[ $(wc -l <"$1") -ge 3 ]]; do sleep 0.1; done' _ "$HOME/herdr-invocations"
[[ "$(wc -l <"$HOME/herdr-invocations")" -eq 3 ]]
[[ "$(tmux -L "$tmux_socket" list-windows -t "${tm_scope}-hub" -F '#{window_name}' | sort)" == $'herdr\nnotes\nshell' ]]
[[ "$(tmux -L "$tmux_socket" list-windows -t "${tm_scope}-pocket" -F '#{window_name}' | sort)" == $'herdr\nshell' ]]
[[ "$(tmux -L "$tmux_socket" display-message -p -t "${tm_scope}-hub:notes" '#{pane_id}')" == "$notes_pane" ]]

other_tm_workspace="$HOME/tm-other-workspace"
mkdir -p "$other_tm_workspace"
other_tm_hash="$(printf '%s' "$other_tm_workspace" | sha256sum)"
other_tm_hash="${other_tm_hash%% *}"
other_tm_scope="tm-${other_tm_workspace##*/}-${other_tm_hash:0:8}"
tmux -L "$tmux_socket" send-keys -t "$controller_pane" "tm --no-attach '$other_tm_workspace'; print \$? >'$other_tm_workspace/status'; tmux wait-for -S tm-other" C-m
timeout 20 tmux -L "$tmux_socket" wait-for tm-other
[[ "$(cat "$other_tm_workspace/status")" == 0 ]]
for role in hub pocket dash; do
	[[ "$(tmux -L "$tmux_socket" show-option -qv -t "${other_tm_scope}-${role}" @tm-root)" == "$other_tm_workspace" ]]
done

cat >"$workspace/.tw.yml" <<YAML
left: touch '$workspace/config-left'
top: touch '$workspace/config-top'
bottom: touch '$workspace/config-bottom'
right: touch '$workspace/config-right'
YAML
tmux -L "$tmux_socket" send-keys -t "$controller_pane" "tw '$workspace'; print \$? >'$workspace/config-status'; tmux wait-for -S tw-config" C-m
timeout 20 tmux -L "$tmux_socket" wait-for tw-config
[[ "$(cat "$workspace/config-status")" == 0 ]]
timeout 20 bash -c 'until [[ -e "$1/config-left" && -e "$1/config-top" && -e "$1/config-bottom" && -e "$1/config-right" ]]; do sleep 0.1; done' _ "$workspace"
[[ "$(tmux -L "$tmux_socket" display-message -p -t controller:tw_workspace '#{window_panes}')" == 4 ]]
[[ "$(tmux -L "$tmux_socket" display-message -p -t controller:tw_workspace '#{pane_current_path} #{pane_active}')" == "$workspace 1" ]]
tmux -L "$tmux_socket" kill-window -t controller:tw_workspace

rm "$workspace/.tw.yml"
tmux -L "$tmux_socket" send-keys -t "$controller_pane" "tw '$workspace' \"touch '$workspace/left-started'\" \"touch '$workspace/top-started'\" \"touch '$workspace/bottom-started'\" \"touch '$workspace/right-started'\"; print \$? >'$workspace/position-status'; tmux wait-for -S tw-position" C-m
timeout 20 tmux -L "$tmux_socket" wait-for tw-position
[[ "$(cat "$workspace/position-status")" == 0 ]]
timeout 20 bash -c 'until [[ -e "$1/left-started" && -e "$1/top-started" && -e "$1/bottom-started" && -e "$1/right-started" ]]; do sleep 0.1; done' _ "$workspace"
[[ "$(tmux -L "$tmux_socket" display-message -p -t controller:tw_workspace '#{window_panes}')" == 4 ]]
tmux -L "$tmux_socket" kill-window -t controller:tw_workspace

[[ "$(tmux -L "$tmux_socket" show-options -gqv base-index)" == 1 ]]
[[ "$(tmux -L "$tmux_socket" show-options -gwqv pane-base-index)" == 1 ]]
[[ "$(tmux -L "$tmux_socket" show-options -gqv renumber-windows)" == off ]]
[[ "$(tmux -L "$tmux_socket" show-options -gqv status-justify)" == left ]]
tmux -L "$tmux_socket" show-options -gqv status-right | grep -Fq '%H:%M'
tmux -L "$tmux_socket" kill-server
trap - EXIT

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
