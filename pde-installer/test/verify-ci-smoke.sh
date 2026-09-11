#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
HOME_DIR="$(mktemp -d)"
BIN_DIR="$HOME_DIR/.local/bin"
INSTALLER="$BIN_DIR/pde-installer"

trap 'rm -rf "$HOME_DIR"' EXIT
mkdir -p "$BIN_DIR"
go build -C "$REPO_ROOT/pde-installer" -o "$INSTALLER" .
root_before="$(tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 -cf - -C "$HOME_DIR" . | sha256sum)"

if root_output="$(sudo -n env HOME="$HOME_DIR" "$INSTALLER" config --repo-root "$REPO_ROOT" 2>&1)"; then
	printf 'root config unexpectedly succeeded\n' >&2
	exit 1
fi
if [[ "$root_output" != *"config refuses UID 0"* ]]; then
	printf 'root config did not report UID rejection:\n%s\n' "$root_output" >&2
	exit 1
fi
root_after="$(tar --sort=name --mtime='UTC 1970-01-01' --owner=0 --group=0 -cf - -C "$HOME_DIR" . | sha256sum)"
[[ "$root_before" == "$root_after" ]]

HOME="$HOME_DIR" PATH="$BIN_DIR:$PATH" "$SCRIPT_DIR/verify.sh"
