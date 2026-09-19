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
cat >"$HOME/.local/bin/herdr" <<'SCRIPT'
#!/usr/bin/env bash
printf 'config=%s args=%s\n' "${HERDR_CONFIG_PATH:-}" "$*" >>"$HOME/herdr-invocations"
exec sleep 300
SCRIPT
chmod 755 "$HOME/.local/bin/herdr"
: >"$HOME/herdr-invocations"
cat >"$HOME/.config/pde/tw.yml" <<YAML
herdr: printf 'herdr\n' >> '$workspace/startups'
wallace: printf 'wallace\n' >> '$workspace/startups'
shell: ""
YAML
chmod 600 "$HOME/.config/pde/tw.yml"
: >"$workspace/startups"

tm_workspace="$HOME/tm-workspace"
mkdir -p "$tm_workspace"
tm_hash="$(printf '%s' "$tm_workspace" | sha256sum)"
tm_hash="${tm_hash%% *}"
tm_scope="tm-${tm_workspace##*/}-${tm_hash:0:8}"
tmux -L "$tmux_socket" -f "$HOME/.tmux.conf" new-session -d -s controller -n shell -c "$tm_workspace" \
	"zsh -ic 'if tm --no-attach \"$tm_workspace\"; then print 0 >\"$tm_workspace/status\"; else print 1 >\"$tm_workspace/status\"; fi; tmux wait-for -S tm-ready; exec zsh'"
timeout 20 tmux -L "$tmux_socket" wait-for tm-ready
[[ "$(cat "$tm_workspace/status")" == 0 ]]
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
controller_pane="$(tmux -L "$tmux_socket" display-message -p -t controller:shell '#{pane_id}')"
tmux -L "$tmux_socket" send-keys -t "$controller_pane" "tm --no-attach '$tm_workspace'; print \$? >'$tm_workspace/repeat-status'; tmux wait-for -S tm-repeat" C-m
timeout 20 tmux -L "$tmux_socket" wait-for tm-repeat
[[ "$(cat "$tm_workspace/repeat-status")" == 0 ]]
[[ "$(tmux -L "$tmux_socket" display-message -p -t "${tm_scope}-hub:herdr" '#{pane_id}')" == "$hub_pane" ]]

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

tmux -L "$tmux_socket" -f "$HOME/.tmux.conf" new-session -d -x 46 -y 22 -s workspace -n zsh -c "$HOME" \
	"zsh -ic 'if tw init \"$workspace\"; then print 0 >\"$workspace/init-status\"; else print 1 >\"$workspace/init-status\"; fi; tmux wait-for -S tw-ready; exec zsh'"
timeout 20 tmux -L "$tmux_socket" wait-for tw-ready
if [[ "$(cat "$workspace/init-status")" != 0 ]]; then
	tmux -L "$tmux_socket" list-windows -t workspace >&2
	tmux -L "$tmux_socket" capture-pane -p -t workspace:shell >&2
	exit 1
fi
timeout 20 bash -c 'until [[ -f "$1" && $(wc -l <"$1") -eq 2 ]]; do sleep 0.1; done' _ "$workspace/startups"
[[ "$(sort "$workspace/startups")" == $'herdr\nwallace' ]]
[[ "$(tmux -L "$tmux_socket" list-windows -t workspace -F '#{window_index}:#{window_name}')" == $'1:herdr\n2:wallace\n3:shell' ]]
for role in herdr wallace shell; do
	[[ "$(tmux -L "$tmux_socket" display-message -p -t workspace:$role '#{pane_current_path}')" == "$workspace" ]]
done
[[ "$(tmux -L "$tmux_socket" list-windows -t workspace -F '#{window_name}:#{window_width}x#{window_height}' | grep '^herdr:')" == 'herdr:46x22' ]]
[[ "$(tmux -L "$tmux_socket" show-option -qv -t workspace @tw-root)" == "$workspace" ]]

shell_pane="$(tmux -L "$tmux_socket" display-message -p -t workspace:shell '#{pane_id}')"
tmux -L "$tmux_socket" select-window -t workspace:shell
tmux -L "$tmux_socket" send-keys -t "$shell_pane" "tw '$workspace' \"touch '$workspace/left-started'\" \"touch '$workspace/top-started'\" \"touch '$workspace/bottom-started'\" \"touch '$workspace/right-started'\" && touch '$workspace/open-succeeded'" C-m
timeout 20 bash -c 'until [[ -e "$1/open-succeeded" && -e "$1/left-started" && -e "$1/top-started" && -e "$1/bottom-started" && -e "$1/right-started" ]]; do sleep 0.1; done' _ "$workspace"
[[ "$(tmux -L "$tmux_socket" display-message -p -t workspace:tw_workspace '#{window_panes}')" == 4 ]]
[[ "$(tmux -L "$tmux_socket" display-message -p -t workspace:tw_workspace '#{pane_current_path} #{pane_active}')" == "$workspace 1" ]]
tmux -L "$tmux_socket" kill-window -t workspace:tw_workspace
tmux -L "$tmux_socket" select-window -t workspace:shell
tmux -L "$tmux_socket" send-keys -t "$shell_pane" "tw && touch '$workspace/open-default-succeeded'" C-m
timeout 20 bash -c 'until [[ -e "$1" ]]; do sleep 0.1; done' _ "$workspace/open-default-succeeded"
[[ "$(tmux -L "$tmux_socket" display-message -p -t workspace:tw_workspace '#{window_panes}')" == 4 ]]
tmux -L "$tmux_socket" kill-window -t workspace:tw_workspace

tmux -L "$tmux_socket" new-window -d -t workspace -n notes -c "$workspace"
tmux -L "$tmux_socket" split-window -d -t workspace:herdr -c "$workspace"
tmux -L "$tmux_socket" kill-window -t workspace:wallace
tmux -L "$tmux_socket" send-keys -t "$shell_pane" "tw init '$workspace' || touch '$workspace/repeat-failed'" C-m
timeout 20 bash -c 'until [[ -f "$1" && $(wc -l <"$1") -eq 3 ]]; do sleep 0.1; done' _ "$workspace/startups"
[[ ! -e "$workspace/repeat-failed" ]]
[[ "$(sort "$workspace/startups")" == $'herdr\nwallace\nwallace' ]]
[[ "$(tmux -L "$tmux_socket" list-windows -t workspace -F '#{window_name}' | sort)" == $'herdr\nnotes\nshell\nwallace' ]]
[[ "$(tmux -L "$tmux_socket" display-message -p -t workspace:herdr '#{window_panes}')" == 2 ]]
[[ "$(tmux -L "$tmux_socket" show-option -qv -t workspace @tw-root)" == "$workspace" ]]

other_workspace="$HOME/tw-other"
mkdir -p "$other_workspace"
tmux -L "$tmux_socket" send-keys -t "$shell_pane" "tw init '$other_workspace' && touch '$workspace/root-unexpected'; tmux wait-for -S tw-root" C-m
timeout 20 tmux -L "$tmux_socket" wait-for tw-root
[[ ! -e "$workspace/root-unexpected" ]]
[[ "$(tmux -L "$tmux_socket" show-option -qv -t workspace @tw-root)" == "$workspace" ]]

uninitialized="$HOME/tw-uninitialized"
mkdir -p "$uninitialized"
tmux -L "$tmux_socket" new-session -d -s uninitialized -n herdr -x 46 -y 22 -c "$uninitialized" \
	"zsh -ic \"tw '$uninitialized' && touch '$uninitialized/opened'; tmux wait-for -S tw-uninitialized; exec zsh\""
timeout 20 tmux -L "$tmux_socket" wait-for tw-uninitialized
[[ -e "$uninitialized/opened" ]]
[[ -z "$(tmux -L "$tmux_socket" show-option -qv -t uninitialized @tw-root)" ]]
[[ "$(tmux -L "$tmux_socket" display-message -p -t uninitialized:tw_uninitialized '#{window_panes}')" == 4 ]]

project_config="$HOME/tw-project-config"
mkdir -p "$project_config"
printf 'herdr: touch %s/project-config-unexpected\nwallace: ""\nshell: ""\n' "$project_config" >"$project_config/.tw.yml"
tmux -L "$tmux_socket" new-session -d -s project-config -x 46 -y 22 -c "$project_config" \
	"zsh -ic \"tw init '$project_config' && touch '$project_config/initialized'; tmux wait-for -S tw-project; exec zsh\""
timeout 20 tmux -L "$tmux_socket" wait-for tw-project
[[ -e "$project_config/initialized" ]]
[[ ! -e "$project_config/project-config-unexpected" ]]
[[ "$(tmux -L "$tmux_socket" show-option -qv -t project-config @tw-root)" == "$project_config" ]]

tmux -L "$tmux_socket" new-session -d -s duplicate -n herdr -x 46 -y 22 -c "$workspace" \
	"zsh -ic 'tmux wait-for tw-duplicate-start; tw init \"$workspace\" && touch \"$workspace/duplicate-unexpected\"; tmux wait-for -S tw-duplicate; exec zsh'"
tmux -L "$tmux_socket" new-window -d -t duplicate:2 -n herdr -c "$workspace"
tmux -L "$tmux_socket" wait-for -S tw-duplicate-start
timeout 20 tmux -L "$tmux_socket" wait-for tw-duplicate
[[ ! -e "$workspace/duplicate-unexpected" ]]
[[ -z "$(tmux -L "$tmux_socket" show-option -qv -t duplicate @tw-root)" ]]

insecure_config="$HOME/tw-insecure-config"
mkdir -p "$insecure_config"
chmod 644 "$HOME/.config/pde/tw.yml"
tmux -L "$tmux_socket" new-session -d -s insecure-config -x 46 -y 22 -c "$insecure_config" \
	"zsh -ic \"tw init '$insecure_config' && touch '$insecure_config/unexpected'; tmux wait-for -S tw-insecure; exec zsh\""
timeout 20 tmux -L "$tmux_socket" wait-for tw-insecure
[[ ! -e "$insecure_config/unexpected" ]]
[[ -z "$(tmux -L "$tmux_socket" show-option -qv -t insecure-config @tw-root)" ]]
[[ "$(tmux -L "$tmux_socket" list-windows -t insecure-config -F '#{window_index}' | wc -l)" -eq 1 ]]
chmod 600 "$HOME/.config/pde/tw.yml"

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
