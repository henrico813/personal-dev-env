#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

[[ "$(id -u)" -ne 0 ]]
[[ -x /usr/bin/sudo ]]

help="$(pde-installer --help)"
for command_name in install doctor list; do
	[[ "$help" == *"  $command_name "* ]]
done
for command_name in update config; do
	[[ "$help" != *"  $command_name "* ]]
	! pde-installer "$command_name" --repo-root "$REPO_ROOT"
done
! pde-installer install --profile terminal --repo-root "$REPO_ROOT"
[[ "$help" != *"completion"* ]]

mkdir -p "$HOME/.config/pde"
printf '{"profile":"full"}\n' >"$HOME/.config/pde/config.json"

before="$(tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 -cf - -C "$HOME" . | sha256sum)"
pde-installer install --dry-run --repo-root "$REPO_ROOT"
after="$(tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 -cf - -C "$HOME" . | sha256sum)"
[[ "$before" == "$after" ]]

pde-installer doctor --repo-root "$REPO_ROOT"
inventory="$(pde-installer list --repo-root "$REPO_ROOT")"
for item in zsh unzip tmux aqua neovim go rust node keychain opencode-ai planner blink.cmp FiraCode repository-config ai-config; do
	[[ "$inventory" == *$'\t'"$item"$'\t'* ]]
done

for forbidden in /opt/pde /usr/local/bin/chezmoi "$HOME/.nvm"; do
	[[ ! -e "$forbidden" ]]
done

printf 'PASS: rootless installer smoke on %s (full installation not exercised)\n' "$(. /etc/os-release && printf '%s' "$PRETTY_NAME")"
