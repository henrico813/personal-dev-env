#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

[[ "$(id -u)" -ne 0 ]]
[[ ! -e /usr/bin/sudo ]]

help="$(pde-installer --help)"
for command_name in install update doctor list config; do
	[[ "$help" == *"  $command_name "* ]]
done
[[ "$help" != *"completion"* ]]

mkdir -p "$HOME/.local/share/aquaproj-aqua/bin" "$HOME/.config/opencode" "$HOME/.config/pde"
install -m 0755 "$REPO_ROOT/pde-installer/test/fixtures/chezmoi" "$HOME/.local/share/aquaproj-aqua/bin/chezmoi"
printf 'preserve-me\n' >"$HOME/.config/opencode/smoke-marker"
printf 'deprecated=true\n' >"$HOME/.config/pde/paths.env"

before="$(tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 -cf - -C "$HOME" . | sha256sum)"
pde-installer install --dry-run --repo-root "$REPO_ROOT"
pde-installer update --dry-run --repo-root "$REPO_ROOT"
pde-installer config --dry-run --repo-root "$REPO_ROOT"
after="$(tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 -cf - -C "$HOME" . | sha256sum)"
[[ "$before" == "$after" ]]

pde-installer config --repo-root "$REPO_ROOT"
cmp -s "$REPO_ROOT/chezmoi/dot_zshrc" "$HOME/.zshrc"
[[ ! -e "$HOME/.config/pde/paths.env" ]]
[[ "$(cat "$HOME/.config/opencode/smoke-marker")" == "preserve-me" ]]
configured="$(tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 -cf - -C "$HOME" . | sha256sum)"
pde-installer config --repo-root "$REPO_ROOT"
rerun="$(tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 -cf - -C "$HOME" . | sha256sum)"
[[ "$configured" == "$rerun" ]]

failure_repo="$(mktemp -d)"
trap 'rm -rf "$failure_repo"' EXIT
cp -a "$REPO_ROOT/chezmoi" "$failure_repo/chezmoi"
mkdir -p "$failure_repo/planner" "$failure_repo/pde-installer"
cp "$REPO_ROOT/planner/go.mod" "$failure_repo/planner/go.mod"
cp "$REPO_ROOT/pde-installer/go.mod" "$failure_repo/pde-installer/go.mod"
: >"$failure_repo/chezmoi/.pde-smoke-fail"
printf 'restore-content\n' >"$HOME/.zshrc"
printf 'restore-marker\n' >"$HOME/.config/opencode/transaction-marker"
if pde-installer config --repo-root "$failure_repo"; then
	printf 'expected config transaction failure\n' >&2
	exit 1
fi
[[ "$(cat "$HOME/.zshrc")" == "restore-content" ]]
[[ "$(cat "$HOME/.config/opencode/transaction-marker")" == "restore-marker" ]]
[[ ! -e "$HOME/.config/pde/smoke-created" ]]

pde-installer doctor --repo-root "$REPO_ROOT"
inventory="$(pde-installer list --repo-root "$REPO_ROOT")"
for item in shells/zsh aqua opencode-ai planner blink.cmp FiraCode repository-config ai-config; do
	[[ "$inventory" == *$'\t'"$item"$'\t'* ]]
done

for forbidden in /opt/pde /usr/local/bin/chezmoi "$HOME/.nvm"; do
	[[ ! -e "$forbidden" ]]
done

printf 'PASS: rootless config transaction smoke on %s (full installation not exercised)\n' "$(. /etc/os-release && printf '%s' "$PRETTY_NAME")"
